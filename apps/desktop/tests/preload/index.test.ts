import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import {
  createDesktopWindowState,
  createDesktopStreamOpenErrorResponse,
  createDesktopStreamOpenSuccessResponse,
  DESKTOP_IPC_SCHEMA,
  DESKTOP_WINDOW_CHANNELS,
  getDesktopStreamResponseChannel,
} from "../../packages/shared/src/ipc.js";

type MockIpcRendererEvent = Pick<Electron.IpcRendererEvent, "ports">;

const hoisted = vi.hoisted(() => {
  const listeners = new Map<string, Set<(event: MockIpcRendererEvent, payload: unknown) => void>>();
  return {
    exposedApi: null as Window["desktopApi"] | null,
    exposedStreamsApi: null as Window["desktopStreams"] | null,
    exposedWindowApi: null as Window["desktopWindow"] | null,
    ipcRenderer: {
      invoke: vi.fn(),
      on: vi.fn(
        (channel: string, listener: (event: MockIpcRendererEvent, payload: unknown) => void) => {
          const existing = listeners.get(channel) ?? new Set();
          existing.add(listener);
          listeners.set(channel, existing);
        },
      ),
      once: vi.fn((channel: string, listener: (event: MockIpcRendererEvent) => void) => {
        const existing = listeners.get(channel) ?? new Set();
        existing.add(listener);
        listeners.set(channel, existing);
      }),
      removeListener: vi.fn((channel: string, listener: (event: MockIpcRendererEvent) => void) => {
        listeners.get(channel)?.delete(listener);
      }),
      send: vi.fn(),
    },
    listeners,
  };
});

vi.mock("electron", () => ({
  contextBridge: {
    exposeInMainWorld: vi.fn(
      (
        key: string,
        api: Window["desktopApi"] | Window["desktopStreams"] | Window["desktopWindow"],
      ) => {
        if (key === "desktopApi") {
          hoisted.exposedApi = api as Window["desktopApi"];
          return;
        }
        if (key === "desktopStreams") {
          hoisted.exposedStreamsApi = api as Window["desktopStreams"];
          return;
        }
        hoisted.exposedWindowApi = api as Window["desktopWindow"];
      },
    ),
  },
  ipcRenderer: hoisted.ipcRenderer,
}));

async function flushMicrotasks(turns = 8): Promise<void> {
  for (let index = 0; index < turns; index += 1) {
    await Promise.resolve();
  }
}

function emitStreamResponse(
  channel: string,
  port: MessagePort,
  payload: unknown = createDesktopStreamOpenSuccessResponse(),
): void {
  const listeners = [...(hoisted.listeners.get(channel) ?? [])];
  for (const listener of listeners) {
    listener({ ports: [port] }, payload);
  }
}

function createFakePort<TMessage>() {
  const fakePort = {
    close: vi.fn(),
    emit(message: TMessage) {
      fakePort.onmessage?.({ data: message } as MessageEvent<TMessage>);
    },
    failDecode() {
      fakePort.onmessageerror?.({ data: undefined } as MessageEvent<TMessage>);
    },
    onmessage: null as ((event: MessageEvent<TMessage>) => void) | null,
    onmessageerror: null as ((event: MessageEvent<TMessage>) => void) | null,
    start: vi.fn(),
  };
  return fakePort;
}

describe("desktop preload stream bridge", () => {
  beforeEach(async () => {
    vi.resetModules();
    hoisted.exposedApi = null;
    hoisted.exposedStreamsApi = null;
    hoisted.exposedWindowApi = null;
    hoisted.listeners.clear();
    hoisted.ipcRenderer.invoke.mockReset();
    hoisted.ipcRenderer.invoke.mockImplementation(async (channel: string) => {
      if (channel === DESKTOP_WINDOW_CHANNELS.getState) {
        return createDesktopWindowState("macos");
      }
      return undefined;
    });
    hoisted.ipcRenderer.on.mockClear();
    hoisted.ipcRenderer.once.mockClear();
    hoisted.ipcRenderer.removeListener.mockClear();
    hoisted.ipcRenderer.send.mockReset();
    await import("../../packages/preload/src/index.js");
    await flushMicrotasks();
  });

  afterEach(() => {
    hoisted.exposedApi = null;
    hoisted.exposedStreamsApi = null;
    hoisted.exposedWindowApi = null;
    hoisted.listeners.clear();
  });

  it("resolves concurrent same-kind stream subscriptions only from their matching replies", async () => {
    if (hoisted.exposedStreamsApi == null) {
      throw new Error("expected preload streams API exposure");
    }

    const portA = createFakePort();
    const portB = createFakePort();

    const promiseA = hoisted.exposedStreamsApi.subscribeRunStream("session-a", null, vi.fn());
    const promiseB = hoisted.exposedStreamsApi.subscribeRunStream("session-b", null, vi.fn());

    expect(hoisted.ipcRenderer.send).toHaveBeenCalledTimes(2);
    const firstRequest = hoisted.ipcRenderer.send.mock.calls[0]?.[1] as {
      requestId: string;
      args: unknown[];
    };
    const secondRequest = hoisted.ipcRenderer.send.mock.calls[1]?.[1] as {
      requestId: string;
      args: unknown[];
    };
    expect(firstRequest.args).toEqual(["session-a", null]);
    expect(secondRequest.args).toEqual(["session-b", null]);
    expect(firstRequest.requestId).not.toBe(secondRequest.requestId);

    let promiseASettled = false;
    void promiseA.then(() => {
      promiseASettled = true;
    });

    emitStreamResponse(
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, secondRequest.requestId),
      portB as unknown as MessagePort,
    );
    const unsubscribeB = await promiseB;
    expect(typeof unsubscribeB).toBe("function");
    await flushMicrotasks();
    expect(promiseASettled).toBe(false);

    emitStreamResponse(
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, firstRequest.requestId),
      portA as unknown as MessagePort,
    );
    const unsubscribeA = await promiseA;
    expect(typeof unsubscribeA).toBe("function");
    expect(
      hoisted.listeners.get(
        getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, firstRequest.requestId),
      )?.size ?? 0,
    ).toBe(0);
    expect(
      hoisted.listeners.get(
        getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, secondRequest.requestId),
      )?.size ?? 0,
    ).toBe(0);

    unsubscribeA();
    unsubscribeB();
  });

  it("rejects the stream subscribe immediately when main replies with an explicit attach error", async () => {
    if (hoisted.exposedStreamsApi == null) {
      throw new Error("expected preload streams API exposure");
    }

    const promise = hoisted.exposedStreamsApi.subscribeRunStream("session-a", null, vi.fn());
    const firstRequest = hoisted.ipcRenderer.send.mock.calls[0]?.[1] as {
      requestId: string;
    };

    emitStreamResponse(
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, firstRequest.requestId),
      createFakePort() as unknown as MessagePort,
      createDesktopStreamOpenErrorResponse("attach failed"),
    );

    await expect(promise).rejects.toThrow("attach failed");
  });

  it("fans out one underlying stream per session and closes it after the last unsubscribe", async () => {
    if (hoisted.exposedStreamsApi == null) {
      throw new Error("expected preload streams API exposure");
    }

    const port = createFakePort<{ stream: "agentStream"; status: "ready" }>();
    const listenerA = vi.fn();
    const listenerB = vi.fn();
    const promiseA = hoisted.exposedStreamsApi.subscribeAgentStream(
      "session-agent",
      null,
      listenerA,
    );
    const promiseB = hoisted.exposedStreamsApi.subscribeAgentStream(
      "session-agent",
      null,
      listenerB,
    );
    expect(hoisted.ipcRenderer.send).toHaveBeenCalledTimes(1);

    const request = hoisted.ipcRenderer.send.mock.calls.at(-1)?.[1] as {
      requestId: string;
      args: unknown[];
    };

    expect(request.args).toEqual(["session-agent", null]);

    emitStreamResponse(
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openAgentStream, request.requestId),
      port as unknown as MessagePort,
    );
    const unsubscribeA = await promiseA;
    const unsubscribeB = await promiseB;

    port.emit({ stream: "agentStream", status: "ready" });
    expect(listenerA).toHaveBeenCalledWith({ stream: "agentStream", status: "ready" });
    expect(listenerB).toHaveBeenCalledWith({ stream: "agentStream", status: "ready" });
    expect(port.start).toHaveBeenCalledTimes(1);

    unsubscribeA();
    expect(port.close).not.toHaveBeenCalled();
    unsubscribeB();
    expect(port.close).toHaveBeenCalledTimes(1);
  });

  it("keeps a renderer snapshot of desktop window state and updates subscribers from main events", async () => {
    if (hoisted.exposedWindowApi == null) {
      throw new Error("expected desktopWindow API exposure");
    }

    expect(hoisted.exposedWindowApi.getSnapshot()).toEqual(createDesktopWindowState("macos"));

    const listener = vi.fn();
    const unsubscribe = hoisted.exposedWindowApi.subscribe(listener);
    const nextState = createDesktopWindowState("macos", {
      isFocused: false,
      isMaximized: true,
    });
    const stateListeners = [
      ...(hoisted.listeners.get(DESKTOP_WINDOW_CHANNELS.stateDidChange) ?? []),
    ];

    for (const stateListener of stateListeners) {
      stateListener({ ports: [] }, nextState);
    }

    expect(listener).toHaveBeenCalledTimes(1);
    expect(hoisted.exposedWindowApi.getSnapshot()).toEqual(nextState);

    unsubscribe();
    for (const stateListener of stateListeners) {
      stateListener({ ports: [] }, createDesktopWindowState("macos"));
    }
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("routes window control actions through the dedicated window channels", async () => {
    if (hoisted.exposedWindowApi == null) {
      throw new Error("expected desktopWindow API exposure");
    }

    hoisted.ipcRenderer.invoke.mockImplementation(async (channel: string) => {
      if (channel === DESKTOP_WINDOW_CHANNELS.close) {
        return undefined;
      }
      if (channel === DESKTOP_WINDOW_CHANNELS.minimize) {
        return createDesktopWindowState("macos", { isFocused: false });
      }
      if (channel === DESKTOP_WINDOW_CHANNELS.toggleMaximize) {
        return createDesktopWindowState("macos", { isMaximized: true });
      }
      return createDesktopWindowState("macos");
    });

    await expect(hoisted.exposedWindowApi.minimize()).resolves.toEqual(
      createDesktopWindowState("macos", { isFocused: false }),
    );
    await expect(hoisted.exposedWindowApi.toggleMaximize()).resolves.toEqual(
      createDesktopWindowState("macos", { isMaximized: true }),
    );
    await expect(hoisted.exposedWindowApi.close()).resolves.toBeUndefined();

    expect(hoisted.ipcRenderer.invoke).toHaveBeenCalledWith(DESKTOP_WINDOW_CHANNELS.minimize);
    expect(hoisted.ipcRenderer.invoke).toHaveBeenCalledWith(DESKTOP_WINDOW_CHANNELS.toggleMaximize);
    expect(hoisted.ipcRenderer.invoke).toHaveBeenCalledWith(DESKTOP_WINDOW_CHANNELS.close);
  });
});
