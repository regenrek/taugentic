import { contextBridge, ipcRenderer } from "electron";

import {
  createDesktopApi,
  createDesktopStreamOpenRequest,
  createDesktopWindowState,
  DESKTOP_IPC_SCHEMA,
  DESKTOP_WINDOW_CHANNELS,
  getDesktopStreamResponseChannel,
  parseDesktopStreamOpenResponse,
  parseDesktopWindowState,
  resolveDesktopWindowPlatform,
  type ActivityCursor,
  type DesktopWindowApi,
  type DesktopWindowState,
  type DesktopStreamsApi,
  type DesktopStreamErrorListener,
  type DesktopStreamListener,
  type DesktopStreamMethod,
  type DesktopStreamPort,
  type DesktopStreamSpec,
  type DesktopStreamUnsubscribe,
  type DaemonEventCursor,
  type SessionId,
  type RunStreamMessage,
  type RunEventStreamMessage,
  type ApprovalStreamMessage,
  type ArtifactStreamMessage,
  type AgentStreamMessage,
} from "@taugentic/desktop-shared";

const STREAM_OPEN_TIMEOUT_MS = 5_000;
let nextStreamRequestId = 1;
const desktopWindowListeners = new Set<() => void>();
let desktopWindowState = createDesktopWindowState(resolveDesktopWindowPlatform(process.platform));

interface StreamSubscriber<Message> {
  readonly onError?: DesktopStreamErrorListener;
  readonly onMessage: DesktopStreamListener<Message>;
}

interface PreloadStreamEntry<Message, Cursor> {
  readonly cursorKey: string;
  readonly method: DesktopStreamMethod;
  openPromise: Promise<void>;
  readonly sessionId: SessionId;
  readonly subscribers: Set<StreamSubscriber<Message>>;
  closed: boolean;
  port: DesktopStreamPort<Message> | null;
  requestedCursor: Cursor | null;
}

const preloadStreamEntries = new Map<string, PreloadStreamEntry<unknown, unknown>>();

function openStream<Message = unknown>(
  spec: DesktopStreamSpec,
  ...args: unknown[]
): Promise<DesktopStreamPort<Message>> {
  return new Promise((resolve, reject) => {
    const request = createDesktopStreamOpenRequest(
      spec,
      `desktop-stream-${nextStreamRequestId++}`,
      args,
    );
    const responseChannel = getDesktopStreamResponseChannel(spec, request.requestId);
    const method = methodFromSpec(spec);
    let settled = false;
    const onResponse = (event: Electron.IpcRendererEvent, payload: unknown) => {
      let response;
      try {
        response = parseDesktopStreamOpenResponse(method, payload);
      } catch (error) {
        finish(error instanceof Error ? error : new Error(String(error)));
        return;
      }
      if (response.status === "error") {
        finish(new Error(response.message));
        return;
      }
      const port = event.ports[0];
      if (!port) {
        finish(new Error(`received no MessagePort on ${responseChannel}`));
        return;
      }
      finish(undefined, port);
    };
    const timeout = setTimeout(() => {
      finish(new Error(`timed out waiting for stream port on ${responseChannel}`));
    }, STREAM_OPEN_TIMEOUT_MS);

    const finish = (error?: Error, port?: DesktopStreamPort<Message>) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timeout);
      ipcRenderer.removeListener(responseChannel, onResponse);
      if (error) {
        reject(error);
        return;
      }
      resolve(port!);
    };

    ipcRenderer.once(responseChannel, onResponse);
    try {
      ipcRenderer.send(spec.requestChannel, request);
    } catch (error) {
      const streamError =
        error instanceof Error
          ? error
          : new Error(`failed to request stream on ${spec.requestChannel}: ${String(error)}`);
      finish(streamError);
    }
  });
}

function closeStreamPort<Message>(port: DesktopStreamPort<Message> | null | undefined): void {
  port?.close?.();
}

function stringifyCursor(cursor: ActivityCursor | DaemonEventCursor | null): string {
  if (cursor === null) {
    return "null";
  }
  if ("daemonInstanceId" in cursor) {
    return `daemon:${cursor.daemonInstanceId}:${cursor.sessionId}:${cursor.sequence.toString()}`;
  }
  return `activity:${cursor.sequence.toString()}`;
}

function makeEntryKey(method: DesktopStreamMethod, sessionId: SessionId): string {
  return `${method}:${sessionId}`;
}

function reportSubscriberError(method: DesktopStreamMethod, error: unknown): void {
  if (typeof console !== "undefined" && typeof console.error === "function") {
    console.error(`desktop preload ${method} subscriber failed`, error);
  }
}

function notifyStreamError<Message, Cursor>(
  key: string,
  entry: PreloadStreamEntry<Message, Cursor>,
  error: Error,
): void {
  const subscribers = Array.from(entry.subscribers);
  for (const subscriber of subscribers) {
    try {
      subscriber.onError?.(error);
    } catch (subscriberError) {
      reportSubscriberError(entry.method, subscriberError);
    }
  }
  teardownStreamEntry(key, entry);
}

function teardownStreamEntry<Message, Cursor>(
  key: string,
  entry: PreloadStreamEntry<Message, Cursor>,
): void {
  if (entry.closed) {
    return;
  }
  entry.closed = true;
  preloadStreamEntries.delete(key);
  if (entry.port !== null) {
    entry.port.onmessage = null;
    entry.port.onmessageerror = null;
  }
  closeStreamPort(entry.port);
  entry.subscribers.clear();
}

function ensurePreloadStreamEntry<Message, Cursor>(
  method: DesktopStreamMethod,
  spec: DesktopStreamSpec,
  sessionId: SessionId,
  afterCursor: Cursor | null,
): PreloadStreamEntry<Message, Cursor> {
  const key = makeEntryKey(method, sessionId);
  const cursorKey = stringifyCursor(afterCursor as ActivityCursor | DaemonEventCursor | null);
  const existing = preloadStreamEntries.get(key) as PreloadStreamEntry<Message, Cursor> | undefined;
  if (existing) {
    if (existing.cursorKey !== cursorKey) {
      throw new Error(
        `desktop preload ${method} already active for ${sessionId} with cursor ${existing.cursorKey}, got ${cursorKey}`,
      );
    }
    return existing;
  }

  const entry: PreloadStreamEntry<Message, Cursor> = {
    cursorKey,
    method,
    openPromise: Promise.resolve(),
    sessionId,
    subscribers: new Set(),
    closed: false,
    port: null,
    requestedCursor: afterCursor,
  };

  entry.openPromise = openStream<Message>(spec, sessionId, afterCursor)
    .then((port) => {
      if (entry.closed) {
        closeStreamPort(port);
        return;
      }

      entry.port = port;
      port.onmessage = (event) => {
        const subscribers = Array.from(entry.subscribers);
        for (const subscriber of subscribers) {
          try {
            subscriber.onMessage(event.data);
          } catch (subscriberError) {
            reportSubscriberError(method, subscriberError);
          }
        }
      };
      port.onmessageerror = () => {
        notifyStreamError(
          key,
          entry,
          new Error(`desktop preload ${method} decode failed for ${sessionId}`),
        );
      };
      port.start?.();
    })
    .catch((error: unknown) => {
      const streamError =
        error instanceof Error
          ? error
          : new Error(`failed to open ${method} for ${sessionId}: ${String(error)}`);
      preloadStreamEntries.delete(key);
      entry.closed = true;
      throw streamError;
    });

  preloadStreamEntries.set(key, entry as PreloadStreamEntry<unknown, unknown>);
  return entry;
}

async function subscribeToPreloadStream<Message, Cursor>(
  method: DesktopStreamMethod,
  spec: DesktopStreamSpec,
  sessionId: SessionId,
  afterCursor: Cursor | null,
  onMessage: DesktopStreamListener<Message>,
  onError?: DesktopStreamErrorListener,
): Promise<DesktopStreamUnsubscribe> {
  const key = makeEntryKey(method, sessionId);
  const entry = ensurePreloadStreamEntry<Message, Cursor>(method, spec, sessionId, afterCursor);
  const subscriber: StreamSubscriber<Message> = {
    onError,
    onMessage,
  };
  entry.subscribers.add(subscriber);
  let active = true;

  const unsubscribe: DesktopStreamUnsubscribe = () => {
    if (!active) {
      return;
    }
    active = false;
    entry.subscribers.delete(subscriber);
    if (entry.subscribers.size === 0) {
      teardownStreamEntry(key, entry);
    }
  };

  try {
    await entry.openPromise;
    if (!active) {
      return unsubscribe;
    }
    return unsubscribe;
  } catch (error) {
    unsubscribe();
    throw error;
  }
}

async function subscribeToDedicatedPreloadStream<Message>(
  spec: DesktopStreamSpec,
  args: unknown[],
  onMessage: DesktopStreamListener<Message>,
  onError?: DesktopStreamErrorListener,
): Promise<DesktopStreamUnsubscribe> {
  const method = methodFromSpec(spec);
  const port = await openStream<Message>(spec, ...args);
  let active = true;
  port.onmessage = (event) => {
    if (!active) {
      return;
    }
    try {
      onMessage(event.data);
    } catch (subscriberError) {
      reportSubscriberError(method, subscriberError);
    }
  };
  port.onmessageerror = () => {
    onError?.(new Error(`desktop preload ${method} decode failed`));
  };
  port.start?.();
  return () => {
    if (!active) {
      return;
    }
    active = false;
    closeStreamPort(port);
  };
}

function methodFromSpec(spec: DesktopStreamSpec): DesktopStreamMethod {
  if (spec.requestChannel === DESKTOP_IPC_SCHEMA.openRunStream.requestChannel) {
    return "openRunStream";
  }
  if (spec.requestChannel === DESKTOP_IPC_SCHEMA.openRunEventStream.requestChannel) {
    return "openRunEventStream";
  }
  if (spec.requestChannel === DESKTOP_IPC_SCHEMA.openApprovalStream.requestChannel) {
    return "openApprovalStream";
  }
  if (spec.requestChannel === DESKTOP_IPC_SCHEMA.openArtifactStream.requestChannel) {
    return "openArtifactStream";
  }
  if (spec.requestChannel === DESKTOP_IPC_SCHEMA.openAgentStream.requestChannel) {
    return "openAgentStream";
  }
  throw new Error(`unknown desktop stream request channel: ${spec.requestChannel}`);
}

function emitDesktopWindowState(nextState: DesktopWindowState): DesktopWindowState {
  desktopWindowState = nextState;
  for (const listener of desktopWindowListeners) {
    listener();
  }
  return desktopWindowState;
}

async function invokeDesktopWindowState(channel: string): Promise<DesktopWindowState> {
  return emitDesktopWindowState(parseDesktopWindowState(await ipcRenderer.invoke(channel)));
}

const desktopApi = createDesktopApi({
  invoke(channel, ...args) {
    return ipcRenderer.invoke(channel, ...args);
  },
});

const desktopStreams: DesktopStreamsApi = {
  subscribeRunStream(sessionId, afterCursor, listener, onError) {
    return subscribeToPreloadStream<RunStreamMessage, ActivityCursor>(
      "openRunStream",
      DESKTOP_IPC_SCHEMA.openRunStream,
      sessionId,
      afterCursor,
      listener,
      onError,
    );
  },
  subscribeRunEventStream(sessionId, runId, afterSeq, listener, onError) {
    return subscribeToDedicatedPreloadStream<RunEventStreamMessage>(
      DESKTOP_IPC_SCHEMA.openRunEventStream,
      [sessionId, runId, afterSeq],
      listener,
      onError,
    );
  },
  subscribeApprovalStream(sessionId, afterCursor, listener, onError) {
    return subscribeToPreloadStream<ApprovalStreamMessage, DaemonEventCursor>(
      "openApprovalStream",
      DESKTOP_IPC_SCHEMA.openApprovalStream,
      sessionId,
      afterCursor,
      listener,
      onError,
    );
  },
  subscribeArtifactStream(sessionId, afterCursor, listener, onError) {
    return subscribeToPreloadStream<ArtifactStreamMessage, DaemonEventCursor>(
      "openArtifactStream",
      DESKTOP_IPC_SCHEMA.openArtifactStream,
      sessionId,
      afterCursor,
      listener,
      onError,
    );
  },
  subscribeAgentStream(sessionId, afterCursor, listener, onError) {
    return subscribeToPreloadStream<AgentStreamMessage, DaemonEventCursor>(
      "openAgentStream",
      DESKTOP_IPC_SCHEMA.openAgentStream,
      sessionId,
      afterCursor,
      listener,
      onError,
    );
  },
};

const desktopWindow: DesktopWindowApi = {
  async close() {
    await ipcRenderer.invoke(DESKTOP_WINDOW_CHANNELS.close);
  },
  getSnapshot() {
    return desktopWindowState;
  },
  minimize() {
    return invokeDesktopWindowState(DESKTOP_WINDOW_CHANNELS.minimize);
  },
  subscribe(listener) {
    desktopWindowListeners.add(listener);
    return () => {
      desktopWindowListeners.delete(listener);
    };
  },
  toggleMaximize() {
    return invokeDesktopWindowState(DESKTOP_WINDOW_CHANNELS.toggleMaximize);
  },
};

ipcRenderer.on(DESKTOP_WINDOW_CHANNELS.stateDidChange, (_event, payload: unknown) => {
  emitDesktopWindowState(parseDesktopWindowState(payload));
});
void invokeDesktopWindowState(DESKTOP_WINDOW_CHANNELS.getState).catch(() => undefined);

contextBridge.exposeInMainWorld("desktopApi", desktopApi);
contextBridge.exposeInMainWorld("desktopStreams", desktopStreams);
contextBridge.exposeInMainWorld("desktopWindow", desktopWindow);
