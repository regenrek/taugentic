import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import { METHOD_DAEMON_EVENT } from "../../packages/shared/generated/index.js";

import {
  attachApprovalStreamPort,
  attachRunStreamPort,
  desktopSessionInvokeHandlers,
  getDaemonSessionOverview,
  getSession as getAttachedSession,
  listDaemonRuns,
  listDaemonSessions,
} from "../../packages/main/src/daemon-session.js";
import { DaemonSessionConnection } from "../../packages/main/src/daemon-session-connection.js";
import {
  DaemonJsonRpcError,
  DAEMON_REQUEST_TIMEOUT_DISABLED,
  DAEMON_REQUEST_TIMEOUT_MS,
  DaemonProtocolError,
  DaemonRequestTimeoutError,
  DaemonRpcUnavailableError,
  DaemonRpcConnection,
} from "../../packages/main/src/daemon-rpc-connection.js";
import { DaemonSessionRequestClient } from "../../packages/main/src/daemon-session-request-client.js";
import { SessionStreamConnection } from "../../packages/main/src/session-stream-connection.js";
import { ProtocolValidationError } from "../../packages/shared/src/validation.js";

const credentialStore = vi.hoisted(() => ({
  loadDesktopClientCredential: vi.fn<() => Promise<string | null>>(async () => null),
  storeDesktopClientCredential: vi.fn(async () => {}),
}));

vi.mock("../../packages/main/src/daemon-client-credential.js", () => credentialStore);

const authorityStore = vi.hoisted(() => ({
  loadDesktopSessionAuthority: vi.fn<() => Promise<string | null>>(
    async () => "session-authority-1session-authority-1",
  ),
  removeDesktopClientSessionAuthorities: vi.fn(async () => {}),
  removeDesktopSessionAuthority: vi.fn(async () => {}),
  storeDesktopSessionAuthority: vi.fn(async () => {}),
}));

vi.mock("../../packages/main/src/daemon-session-authority.js", () => authorityStore);

interface FakePort {
  postMessage: ReturnType<typeof vi.fn>;
  once: (event: string, listener: () => void) => void;
  close: () => void;
}

function createFakePort(): FakePort {
  let closeListener: (() => void) | null = null;
  return {
    postMessage: vi.fn(),
    once: (_event, listener) => {
      closeListener = listener;
    },
    close: () => {
      closeListener?.();
    },
  };
}

function runEvent(sequence: number, detail: string) {
  return {
    daemonInstanceId: "daemon-1",
    sessionId: "session-1",
    sequence: BigInt(sequence),
    occurredAtMs: BigInt(sequence * 10),
    event: {
      run: {
        runId: `run-${sequence}`,
        status: "queued",
        detail,
      },
    },
  };
}

function approvalEvent(sequence: number, reason: string) {
  return {
    daemonInstanceId: "daemon-1",
    sessionId: "session-1",
    sequence: BigInt(sequence),
    occurredAtMs: BigInt(sequence * 10),
    event: {
      approval: {
        phase: "requested",
        request: {
          id: `approval-${sequence}`,
          runId: `run-${sequence}`,
          scope: "processExec",
          reason,
        },
      },
    },
  };
}

function artifactEvent(sequence: number, runId: string) {
  return {
    daemonInstanceId: "daemon-1",
    sessionId: "session-1",
    sequence: BigInt(sequence),
    occurredAtMs: BigInt(sequence * 10),
    event: {
      artifact: {
        artifact: {
          id: `artifact-${sequence}`,
          runId,
          kind: "Patch",
          storagePath: `artifacts/${runId}/artifact-${sequence}.diff`,
        },
      },
    },
  };
}

function agentStreamEvent(sequence: number, delta: string) {
  return {
    daemonInstanceId: "daemon-1",
    sessionId: "session-1",
    sequence: BigInt(sequence),
    occurredAtMs: BigInt(sequence * 10),
    event: {
      agentStream: {
        runId: `run-${sequence}`,
        turnId: `turn-${sequence}`,
        itemId: null,
        fragmentSequence: sequence,
        frame: {
          kind: "assistantMessageDelta",
          delta,
        },
      },
    },
  };
}

function resumeCursor(sequence: number, sessionId = "session-1", daemonInstanceId = "daemon-1") {
  return {
    daemonInstanceId,
    sessionId,
    sequence: BigInt(sequence),
  };
}

function initializeResult(daemonInstanceId = "daemon-2") {
  return {
    daemonInstanceId,
    clientCredential: "credential-1credential-1credential-1",
    daemonVersion: "0.0.1",
    protocolVersion: "2026-04-stage3",
    capabilities: {
      notifications: true,
      eventSubscriptions: true,
    },
  };
}

function attachResult(sessionId = "session-1", daemonInstanceId = "daemon-2") {
  return {
    session: {
      id: sessionId,
      title: "Build daemon app server",
      status: "running",
    },
    sessionAuthority: "session-authority-2session-authority-2",
    latestCursor: {
      daemonInstanceId,
      sessionId,
      sequence: "11",
    },
  };
}

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;

  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, reject, resolve };
}

function createScriptedRpcSocket(connection: any, steps: Array<{ result: unknown } | null>) {
  let writeCount = 0;
  const socket = {
    destroyed: false,
    write: vi.fn((_line: string, callback?: (error?: Error | null) => void) => {
      const step = steps[writeCount] ?? null;
      writeCount += 1;
      callback?.(undefined);
      if (step == null) {
        return true;
      }
      void Promise.resolve().then(() => {
        connection.rpcConnection.handleJsonRpcLine(
          JSON.stringify({
            jsonrpc: "2.0",
            id: writeCount,
            result: step.result,
          }),
        );
      });
      return true;
    }),
    destroy: vi.fn(function (this: { destroyed: boolean }) {
      this.destroyed = true;
    }),
  };
  connection.rpcConnection.socket = socket;
  return socket;
}

describe("daemon session owners", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    credentialStore.loadDesktopClientCredential.mockReset();
    credentialStore.loadDesktopClientCredential.mockImplementation(async () => null);
    credentialStore.storeDesktopClientCredential.mockReset();
    credentialStore.storeDesktopClientCredential.mockImplementation(async () => {});
    authorityStore.loadDesktopSessionAuthority.mockReset();
    authorityStore.loadDesktopSessionAuthority.mockImplementation(
      async () => "session-authority-1session-authority-1",
    );
    authorityStore.removeDesktopClientSessionAuthorities.mockReset();
    authorityStore.removeDesktopClientSessionAuthorities.mockImplementation(async () => {});
    authorityStore.removeDesktopSessionAuthority.mockReset();
    authorityStore.removeDesktopSessionAuthority.mockImplementation(async () => {});
    authorityStore.storeDesktopSessionAuthority.mockReset();
    authorityStore.storeDesktopSessionAuthority.mockImplementation(async () => {});
  });

  it("marks run ports ready without replaying old run history", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();

    session.ensureSubscribedKinds = vi.fn(async () => "ready");
    session.daemonInstanceId = "daemon-1";
    session.handleDaemonEventEnvelope(runEvent(7, "stale run"));

    await session.attachRunPort(attachingPort);

    expect(session.ensureSubscribedKinds).toHaveBeenCalledWith(["run"], null);
    expect(attachingPort.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "ready",
    });
    expect(session.runPorts.has(attachingPort)).toBe(true);
  });

  it("surfaces historyGap on the first run attach when subscribe returns a gap", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();

    session.ensureSubscribedKinds = vi.fn(async () => "historyGap");

    await session.attachRunPort(attachingPort);

    expect(session.ensureSubscribedKinds).toHaveBeenCalledWith(["run"], null);
    expect(attachingPort.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "historyGap",
    });
    expect(session.runPorts.has(attachingPort)).toBe(true);
  });

  it("replays run events that land during the first attach window after ready", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();
    const subscribe = createDeferred<"ready">();

    session.ensureSubscribedKinds = vi.fn(() => subscribe.promise);
    session.daemonInstanceId = "daemon-1";

    const attachPromise = session.attachRunPort(attachingPort);
    session.handleDaemonEventEnvelope(runEvent(7, "attach-window"));

    subscribe.resolve("ready");
    await attachPromise;

    expect(attachingPort.postMessage).toHaveBeenNthCalledWith(1, {
      stream: "runs",
      status: "ready",
    });
    expect(attachingPort.postMessage).toHaveBeenNthCalledWith(2, runEvent(7, "attach-window"));
    expect(session.runPorts.has(attachingPort)).toBe(true);
  });

  it("uses the hydrated run activity cursor for the first live subscribe", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();

    session.ensureConnected = vi.fn(async () => {});
    session.daemonInstanceId = "daemon-1";
    session.sendRequest = vi.fn().mockResolvedValue({
      status: "ready",
      latestCursor: resumeCursor(12),
    });

    await session.attachRunPort(attachingPort, { sequence: 9n });

    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.subscribe",
      {
        kinds: ["run"],
        afterCursor: {
          daemonInstanceId: "daemon-1",
          sessionId: "session-1",
          sequence: "9",
        },
      },
      expect.any(Function),
    );
    expect(attachingPort.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "ready",
    });
  });

  it("marks approval ports ready without replaying old approval history", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();

    session.ensureSubscribedKinds = vi.fn(async () => "ready");
    session.daemonInstanceId = "daemon-1";
    session.handleDaemonEventEnvelope(approvalEvent(7, "stale approval"));

    await session.attachApprovalPort(attachingPort);

    expect(session.ensureSubscribedKinds).toHaveBeenCalledWith(["approval"], null);
    expect(attachingPort.postMessage).toHaveBeenCalledWith({
      stream: "approvals",
      status: "ready",
    });
    expect(session.approvalPorts.has(attachingPort)).toBe(true);
  });

  it("surfaces historyGap on the first approval attach when subscribe returns a gap", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();

    session.ensureSubscribedKinds = vi.fn(async () => "historyGap");

    await session.attachApprovalPort(attachingPort);

    expect(session.ensureSubscribedKinds).toHaveBeenCalledWith(["approval"], null);
    expect(attachingPort.postMessage).toHaveBeenCalledWith({
      stream: "approvals",
      status: "historyGap",
    });
    expect(session.approvalPorts.has(attachingPort)).toBe(true);
  });

  it("marks artifact ports ready without replaying old artifact history", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();

    session.ensureSubscribedKinds = vi.fn(async () => "ready");
    session.daemonInstanceId = "daemon-1";
    session.handleDaemonEventEnvelope(artifactEvent(7, "run-7"));

    await session.attachArtifactPort(attachingPort);

    expect(session.ensureSubscribedKinds).toHaveBeenCalledWith(["artifact"], null);
    expect(attachingPort.postMessage).toHaveBeenCalledWith({
      stream: "artifacts",
      status: "ready",
    });
    expect(session.artifactPorts.has(attachingPort)).toBe(true);
  });

  it("surfaces historyGap on the first artifact attach when subscribe returns a gap", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();

    session.ensureSubscribedKinds = vi.fn(async () => "historyGap");

    await session.attachArtifactPort(attachingPort);

    expect(session.ensureSubscribedKinds).toHaveBeenCalledWith(["artifact"], null);
    expect(attachingPort.postMessage).toHaveBeenCalledWith({
      stream: "artifacts",
      status: "historyGap",
    });
    expect(session.artifactPorts.has(attachingPort)).toBe(true);
  });

  it("marks agent stream ports ready without replaying old history", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();

    session.ensureSubscribedKinds = vi.fn(async () => "ready");
    session.daemonInstanceId = "daemon-1";
    session.handleDaemonEventEnvelope(agentStreamEvent(7, "stale delta"));

    await session.attachAgentStreamPort(attachingPort);

    expect(session.ensureSubscribedKinds).toHaveBeenCalledWith(["agentStream"], null);
    expect(attachingPort.postMessage).toHaveBeenCalledWith({
      latestCursor: resumeCursor(7),
      stream: "agentStream",
      status: "ready",
    });
    expect(session.agentStreamPorts.has(attachingPort)).toBe(true);
  });

  it("surfaces historyGap on the first agent stream attach when subscribe returns a gap", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();

    session.ensureSubscribedKinds = vi.fn(async () => "historyGap");

    await session.attachAgentStreamPort(attachingPort);

    expect(session.ensureSubscribedKinds).toHaveBeenCalledWith(["agentStream"], null);
    expect(attachingPort.postMessage).toHaveBeenCalledWith({
      stream: "agentStream",
      status: "historyGap",
    });
    expect(session.agentStreamPorts.has(attachingPort)).toBe(true);
  });

  it("replays agent stream envelopes that land during the first attach window with full payload", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();
    const subscribe = createDeferred<"ready">();

    session.ensureSubscribedKinds = vi.fn(() => subscribe.promise);
    session.daemonInstanceId = "daemon-1";

    const attachPromise = session.attachAgentStreamPort(attachingPort);
    session.handleDaemonEventEnvelope(agentStreamEvent(7, "attach-window"));

    subscribe.resolve("ready");
    await attachPromise;

    expect(attachingPort.postMessage).toHaveBeenNthCalledWith(1, {
      latestCursor: resumeCursor(7),
      stream: "agentStream",
      status: "ready",
    });
    expect(attachingPort.postMessage).toHaveBeenNthCalledWith(2, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 7n,
      occurredAtMs: 70n,
      event: {
        agentStream: {
          runId: "run-7",
          turnId: "turn-7",
          itemId: null,
          fragmentSequence: 7,
          frame: {
            kind: "assistantMessageDelta",
            delta: "attach-window",
          },
        },
      },
    });
    expect(session.agentStreamPorts.has(attachingPort)).toBe(true);
  });

  it("preserves the daemon event cursor lineage for the first agent stream subscribe", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const attachingPort = createFakePort();
    const afterCursor = resumeCursor(9, "session-1", "daemon-7");

    session.ensureConnected = vi.fn(async () => {});
    session.daemonInstanceId = "daemon-7";
    session.sendRequest = vi.fn().mockResolvedValue({
      status: "ready",
      latestCursor: resumeCursor(12, "session-1", "daemon-7"),
    });

    await session.attachAgentStreamPort(attachingPort, afterCursor);

    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.subscribe",
      {
        kinds: ["agentStream"],
        afterCursor: {
          daemonInstanceId: "daemon-7",
          sessionId: "session-1",
          sequence: "9",
        },
      },
      expect.any(Function),
    );
    expect(attachingPort.postMessage).toHaveBeenCalledWith({
      latestCursor: resumeCursor(12, "session-1", "daemon-7"),
      stream: "agentStream",
      status: "ready",
    });
  });

  it("filters later agent stream consumers by their own cursor on a shared session connection", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const existingPort = createFakePort();
    const laterPort = createFakePort();
    const subscribe = createDeferred<"ready">();

    session.ensureSubscribedKinds = vi.fn(() => subscribe.promise);
    session.daemonInstanceId = "daemon-1";
    session.agentStreamPorts.add(existingPort);
    session.streamDescriptors.agentStream.activePortCursors.set(existingPort, null);

    const attachPromise = session.attachAgentStreamPort(laterPort, resumeCursor(9));
    session.handleDaemonEventEnvelope(agentStreamEvent(7, "before-later-cursor"));
    session.handleDaemonEventEnvelope(agentStreamEvent(10, "after-later-cursor"));

    subscribe.resolve("ready");
    await attachPromise;

    session.handleDaemonEventEnvelope(agentStreamEvent(11, "after-ready"));

    expect(existingPort.postMessage).toHaveBeenNthCalledWith(1, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 7n,
      occurredAtMs: 70n,
      event: {
        agentStream: {
          runId: "run-7",
          turnId: "turn-7",
          itemId: null,
          fragmentSequence: 7,
          frame: {
            kind: "assistantMessageDelta",
            delta: "before-later-cursor",
          },
        },
      },
    });
    expect(existingPort.postMessage).toHaveBeenNthCalledWith(2, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 10n,
      occurredAtMs: 100n,
      event: {
        agentStream: {
          runId: "run-10",
          turnId: "turn-10",
          itemId: null,
          fragmentSequence: 10,
          frame: {
            kind: "assistantMessageDelta",
            delta: "after-later-cursor",
          },
        },
      },
    });
    expect(existingPort.postMessage).toHaveBeenNthCalledWith(3, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 11n,
      occurredAtMs: 110n,
      event: {
        agentStream: {
          runId: "run-11",
          turnId: "turn-11",
          itemId: null,
          fragmentSequence: 11,
          frame: {
            kind: "assistantMessageDelta",
            delta: "after-ready",
          },
        },
      },
    });
    expect(laterPort.postMessage).toHaveBeenNthCalledWith(1, {
      latestCursor: resumeCursor(10),
      stream: "agentStream",
      status: "ready",
    });
    expect(laterPort.postMessage).toHaveBeenNthCalledWith(2, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 10n,
      occurredAtMs: 100n,
      event: {
        agentStream: {
          runId: "run-10",
          turnId: "turn-10",
          itemId: null,
          fragmentSequence: 10,
          frame: {
            kind: "assistantMessageDelta",
            delta: "after-later-cursor",
          },
        },
      },
    });
    expect(laterPort.postMessage).toHaveBeenNthCalledWith(3, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 11n,
      occurredAtMs: 110n,
      event: {
        agentStream: {
          runId: "run-11",
          turnId: "turn-11",
          itemId: null,
          fragmentSequence: 11,
          frame: {
            kind: "assistantMessageDelta",
            delta: "after-ready",
          },
        },
      },
    });
    expect(laterPort.postMessage).not.toHaveBeenCalledWith(
      expect.objectContaining({
        sequence: 7n,
      }),
    );
  });

  it("surfaces historyGap after reconnect instead of replaying subscribe results directly", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const existingPort = createFakePort();

    session.ensureConnected = vi.fn(async () => {});
    session.daemonInstanceId = "daemon-1";
    session.latestCursor = resumeCursor(6);
    session.sendRequest = vi.fn().mockResolvedValue({
      status: "historyGap",
      latestCursor: resumeCursor(8),
    });
    session.runPorts.add(existingPort);

    await session.restoreSubscriptions();

    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.subscribe",
      {
        kinds: ["run"],
        afterCursor: {
          daemonInstanceId: "daemon-1",
          sessionId: "session-1",
          sequence: "6",
        },
      },
      expect.any(Function),
    );
    expect(session.latestCursor).toEqual(resumeCursor(8));
    expect(existingPort.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "historyGap",
    });
  });

  it("surfaces historyGap to all active stream kinds after reconnect", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const runPort = createFakePort();
    const approvalPort = createFakePort();

    session.ensureConnected = vi.fn(async () => {});
    session.daemonInstanceId = "daemon-1";
    session.latestCursor = resumeCursor(6);
    session.sendRequest = vi.fn().mockResolvedValue({
      status: "historyGap",
      latestCursor: resumeCursor(8),
    });
    session.runPorts.add(runPort);
    session.approvalPorts.add(approvalPort);

    await session.restoreSubscriptions();

    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.subscribe",
      {
        kinds: ["run", "approval"],
        afterCursor: {
          daemonInstanceId: "daemon-1",
          sessionId: "session-1",
          sequence: "6",
        },
      },
      expect.any(Function),
    );
    expect(session.latestCursor).toEqual(resumeCursor(8));
    expect(runPort.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "historyGap",
    });
    expect(approvalPort.postMessage).toHaveBeenCalledWith({
      stream: "approvals",
      status: "historyGap",
    });
  });

  it("reattaches the business session before subscribe on stream reconnect", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    const existingPort = createFakePort();
    authorityStore.loadDesktopSessionAuthority
      .mockResolvedValueOnce("session-authority-1session-authority-1")
      .mockResolvedValueOnce("session-authority-2session-authority-2");
    const responses = [
      initializeResult("daemon-2"),
      {
        session: {
          id: "session-9",
          title: "Build daemon app server",
          status: "running",
        },
        sessionAuthority: "session-authority-2session-authority-2",
        latestCursor: {
          daemonInstanceId: "daemon-2",
          sessionId: "session-9",
          sequence: "11",
        },
      },
      {
        status: "ready",
        latestCursor: { daemonInstanceId: "daemon-2", sessionId: "session-9", sequence: "13" },
      },
      initializeResult("daemon-2"),
      {
        session: {
          id: "session-9",
          title: "Build daemon app server",
          status: "running",
        },
        sessionAuthority: "session-authority-3session-authority-3",
        latestCursor: {
          daemonInstanceId: "daemon-2",
          sessionId: "session-9",
          sequence: "15",
        },
      },
      {
        status: "ready",
        latestCursor: { daemonInstanceId: "daemon-2", sessionId: "session-9", sequence: "17" },
      },
    ];

    session.runPorts.add(existingPort);
    session.latestCursor = resumeCursor(12, "session-9", "daemon-2");
    session.connection.request = vi.fn(
      async (_method: string, _params: unknown, parseResult: (value: unknown) => unknown) =>
        parseResult(responses.shift()),
    );
    session.connection.ensureConnected = vi.fn(async function (this: any) {
      await this.initializeConnection();
    });

    await session.restoreSubscriptions();
    await session.restoreSubscriptions();

    expect(session.connection.request).toHaveBeenNthCalledWith(
      1,
      "daemon.initialize",
      expect.any(Object),
      expect.any(Function),
    );
    expect(session.connection.request).toHaveBeenNthCalledWith(
      2,
      "daemon.session.attach",
      {
        sessionId: "session-9",
        sessionAuthority: "session-authority-1session-authority-1",
      },
      expect.any(Function),
    );
    expect(session.connection.request).toHaveBeenNthCalledWith(
      3,
      "daemon.subscribe",
      {
        kinds: ["run"],
        afterCursor: {
          daemonInstanceId: "daemon-2",
          sessionId: "session-9",
          sequence: "11",
        },
      },
      expect.any(Function),
    );
    expect(session.connection.request).toHaveBeenNthCalledWith(
      4,
      "daemon.initialize",
      expect.any(Object),
      expect.any(Function),
    );
    expect(session.connection.request).toHaveBeenNthCalledWith(
      5,
      "daemon.session.attach",
      {
        sessionId: "session-9",
        sessionAuthority: "session-authority-2session-authority-2",
      },
      expect.any(Function),
    );
    expect(session.connection.request).toHaveBeenNthCalledWith(
      6,
      "daemon.subscribe",
      {
        kinds: ["run"],
        afterCursor: {
          daemonInstanceId: "daemon-2",
          sessionId: "session-9",
          sequence: "15",
        },
      },
      expect.any(Function),
    );
    expect(session.latestCursor).toEqual(resumeCursor(17, "session-9", "daemon-2"));
  });

  it("rejects attach when daemon.session.attach returns a different session id", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    const responses = [
      initializeResult("daemon-2"),
      {
        session: {
          id: "session-other",
          title: "Build daemon app server",
          status: "running",
        },
        sessionAuthority: "session-authority-2session-authority-2",
        latestCursor: {
          daemonInstanceId: "daemon-2",
          sessionId: "session-9",
          sequence: "11",
        },
      },
    ];

    session.connection.request = vi.fn(
      async (_method: string, _params: unknown, parseResult: (value: unknown) => unknown) =>
        parseResult(responses.shift()),
    );

    await expect(session.connection.initializeConnection()).rejects.toThrow(
      new DaemonProtocolError(
        "daemon attached wrong session: expected session-9, got session-other",
      ),
    );
    expect(session.connection.request).toHaveBeenCalledTimes(2);
    expect(session.connection.request).toHaveBeenNthCalledWith(
      2,
      "daemon.session.attach",
      {
        sessionId: "session-9",
        sessionAuthority: "session-authority-1session-authority-1",
      },
      expect.any(Function),
    );
  });

  it("purges stale local session authority after terminal daemon.session.attach denial", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    const staleError = new DaemonJsonRpcError(-32_602, "session authority rejected: session-9");
    authorityStore.loadDesktopSessionAuthority
      .mockResolvedValueOnce("session-authority-1session-authority-1")
      .mockResolvedValueOnce(null);

    session.connection.request = vi.fn(async (method: string) => {
      if (method === "daemon.initialize") {
        return initializeResult("daemon-2");
      }
      throw staleError;
    });

    await expect(session.connection.initializeConnection()).rejects.toThrow(staleError);
    expect(authorityStore.removeDesktopSessionAuthority).toHaveBeenCalledWith(
      "desktop-main",
      "session-9",
    );
    await expect(session.connection.initializeConnection()).rejects.toThrow(
      new DaemonProtocolError("missing local session authority for session-9"),
    );
    expect(session.connection.request).toHaveBeenCalledTimes(3);
  });

  it("rejects attach when daemon.session.attach latestCursor has a different session id", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    const responses = [
      initializeResult("daemon-2"),
      {
        session: {
          id: "session-9",
          title: "Build daemon app server",
          status: "running",
        },
        sessionAuthority: "session-authority-2session-authority-2",
        latestCursor: {
          daemonInstanceId: "daemon-2",
          sessionId: "session-other",
          sequence: "11",
        },
      },
    ];

    session.connection.request = vi.fn(
      async (_method: string, _params: unknown, parseResult: (value: unknown) => unknown) =>
        parseResult(responses.shift()),
    );

    await expect(session.connection.initializeConnection()).rejects.toThrow(
      new DaemonProtocolError(
        "daemon.session.attach returned cursor for wrong session: expected session-9, got session-other",
      ),
    );
  });

  it("rejects attach when daemon.session.attach latestCursor has a different daemon instance id", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    const responses = [
      initializeResult("daemon-2"),
      {
        session: {
          id: "session-9",
          title: "Build daemon app server",
          status: "running",
        },
        sessionAuthority: "session-authority-2session-authority-2",
        latestCursor: {
          daemonInstanceId: "daemon-other",
          sessionId: "session-9",
          sequence: "11",
        },
      },
    ];

    session.connection.request = vi.fn(
      async (_method: string, _params: unknown, parseResult: (value: unknown) => unknown) =>
        parseResult(responses.shift()),
    );

    await expect(session.connection.initializeConnection()).rejects.toThrow(
      new DaemonProtocolError(
        "daemon.session.attach returned cursor for wrong daemon instance: expected daemon-2, got daemon-other",
      ),
    );
  });

  it("rejects restore when daemon.subscribe latestCursor has a different session id", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    const existingPort = createFakePort();

    session.ensureConnected = vi.fn(async () => {});
    session.daemonInstanceId = "daemon-2";
    session.latestCursor = resumeCursor(12, "session-9", "daemon-2");
    session.sendRequest = vi.fn().mockResolvedValue({
      status: "ready",
      latestCursor: resumeCursor(13, "session-other", "daemon-2"),
    });
    session.runPorts.add(existingPort);

    await expect(session.restoreSubscriptions()).rejects.toThrow(
      new DaemonProtocolError(
        "daemon.subscribe returned cursor for wrong session: expected session-9, got session-other",
      ),
    );
    expect(existingPort.postMessage).not.toHaveBeenCalledWith({
      stream: "runs",
      status: "historyGap",
    });
    expect(session.latestCursor).toEqual(resumeCursor(12, "session-9", "daemon-2"));
  });

  it("rejects restore when daemon.subscribe latestCursor has a different daemon instance id", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    const existingPort = createFakePort();

    session.ensureConnected = vi.fn(async () => {});
    session.daemonInstanceId = "daemon-2";
    session.latestCursor = resumeCursor(12, "session-9", "daemon-2");
    session.sendRequest = vi.fn().mockResolvedValue({
      status: "ready",
      latestCursor: resumeCursor(13, "session-9", "daemon-other"),
    });
    session.runPorts.add(existingPort);

    await expect(session.restoreSubscriptions()).rejects.toThrow(
      new DaemonProtocolError(
        "daemon.subscribe returned cursor for wrong daemon instance: expected daemon-2, got daemon-other",
      ),
    );
    expect(existingPort.postMessage).not.toHaveBeenCalledWith({
      stream: "runs",
      status: "historyGap",
    });
    expect(session.latestCursor).toEqual(resumeCursor(12, "session-9", "daemon-2"));
  });

  it("rejects first attach when daemon.subscribe latestCursor has a different session id", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    const port = createFakePort();

    session.ensureConnected = vi.fn(async () => {});
    session.daemonInstanceId = "daemon-2";
    session.sendRequest = vi.fn().mockResolvedValue({
      status: "ready",
      latestCursor: resumeCursor(13, "session-other", "daemon-2"),
    });

    await expect(session.attachRunPort(port)).rejects.toThrow(
      new DaemonProtocolError(
        "daemon.subscribe returned cursor for wrong session: expected session-9, got session-other",
      ),
    );
    expect(port.postMessage).not.toHaveBeenCalledWith({
      stream: "runs",
      status: "ready",
    });
    expect(session.runPorts.has(port)).toBe(false);
  });

  it("rejects first attach when daemon.subscribe latestCursor has a different daemon instance id", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    const port = createFakePort();

    session.ensureConnected = vi.fn(async () => {});
    session.daemonInstanceId = "daemon-2";
    session.sendRequest = vi.fn().mockResolvedValue({
      status: "ready",
      latestCursor: resumeCursor(13, "session-9", "daemon-other"),
    });

    await expect(session.attachRunPort(port)).rejects.toThrow(
      new DaemonProtocolError(
        "daemon.subscribe returned cursor for wrong daemon instance: expected daemon-2, got daemon-other",
      ),
    );
    expect(port.postMessage).not.toHaveBeenCalledWith({
      stream: "runs",
      status: "ready",
    });
    expect(session.runPorts.has(port)).toBe(false);
  });

  it("retries restoring subscriptions with backoff while ports remain attached", async () => {
    vi.useFakeTimers();
    const session = new SessionStreamConnection("session-1") as any;
    const port = createFakePort();

    session.runPorts.add(port);
    session.restoreSubscriptions = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(new DaemonRpcUnavailableError("daemon down"))
      .mockResolvedValue(undefined);
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    session.handleTransportTermination(new DaemonRpcUnavailableError("socket closed"));
    await vi.advanceTimersByTimeAsync(250);
    await Promise.resolve();
    expect(session.restoreSubscriptions).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(500);
    await Promise.resolve();
    expect(session.restoreSubscriptions).toHaveBeenCalledTimes(2);
    expect(consoleError).toHaveBeenCalledTimes(1);
  });

  it("stops retrying restore after a terminal protocol failure", async () => {
    vi.useFakeTimers();
    const session = new SessionStreamConnection("session-1") as any;
    const port = createFakePort();

    session.runPorts.add(port);
    session.restoreSubscriptions = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(
        new ProtocolValidationError("DaemonSubscribeResult failed protocol validation"),
      );
    session.connection.dispose = vi.fn();
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    session.handleTransportTermination(new DaemonRpcUnavailableError("socket closed"));
    await vi.advanceTimersByTimeAsync(250);
    await Promise.resolve();
    expect(session.restoreSubscriptions).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(500);
    await Promise.resolve();
    expect(session.restoreSubscriptions).toHaveBeenCalledTimes(1);
    expect(session.connection.dispose).toHaveBeenCalledTimes(1);
    expect(port.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "terminalError",
    });
    expect(consoleError).toHaveBeenCalledWith(
      "stopped restoring persistent daemon subscriptions after terminal failure",
      expect.any(ProtocolValidationError),
    );
  });

  it("stops retrying restore after a terminal protocol failure across all active stream kinds", async () => {
    vi.useFakeTimers();
    const session = new SessionStreamConnection("session-1") as any;
    const runPort = createFakePort();
    const approvalPort = createFakePort();

    session.runPorts.add(runPort);
    session.approvalPorts.add(approvalPort);
    session.restoreSubscriptions = vi
      .fn<() => Promise<void>>()
      .mockRejectedValueOnce(
        new ProtocolValidationError("DaemonSubscribeResult failed protocol validation"),
      );
    session.connection.dispose = vi.fn();
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    session.handleTransportTermination(new DaemonRpcUnavailableError("socket closed"));
    await vi.advanceTimersByTimeAsync(250);
    await Promise.resolve();
    expect(session.restoreSubscriptions).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(500);
    await Promise.resolve();
    expect(session.restoreSubscriptions).toHaveBeenCalledTimes(1);
    expect(session.connection.dispose).toHaveBeenCalledTimes(1);
    expect(runPort.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "terminalError",
    });
    expect(approvalPort.postMessage).toHaveBeenCalledWith({
      stream: "approvals",
      status: "terminalError",
    });
    expect(consoleError).toHaveBeenCalledWith(
      "stopped restoring persistent daemon subscriptions after terminal failure",
      expect.any(ProtocolValidationError),
    );
  });

  it("stops retrying restore after reconnect hits a stale session authority denial", async () => {
    vi.useFakeTimers();
    const session = new SessionStreamConnection("session-9") as any;
    const port = createFakePort();
    const staleError = new DaemonJsonRpcError(-32_602, "session authority rejected: session-9");
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    session.runPorts.add(port);
    session.connection.dispose = vi.fn();
    session.connection.request = vi.fn(
      async (method: string, _params: unknown, parseResult: (value: unknown) => unknown) => {
        if (method === "daemon.initialize") {
          return parseResult(initializeResult("daemon-2"));
        }
        if (method === "daemon.session.attach") {
          throw staleError;
        }
        throw new Error(`unexpected method during reconnect: ${method}`);
      },
    );
    session.ensureConnected = vi.fn(async () => {
      await session.connection.initializeConnection();
    });

    session.handleTransportTermination(new DaemonRpcUnavailableError("socket closed"));
    await vi.advanceTimersByTimeAsync(250);
    await Promise.resolve();

    expect(session.connection.request).toHaveBeenCalledTimes(2);
    expect(session.connection.request).toHaveBeenNthCalledWith(
      1,
      "daemon.initialize",
      expect.any(Object),
      expect.any(Function),
    );
    expect(session.connection.request).toHaveBeenNthCalledWith(
      2,
      "daemon.session.attach",
      {
        sessionId: "session-9",
        sessionAuthority: "session-authority-1session-authority-1",
      },
      expect.any(Function),
    );
    expect(authorityStore.removeDesktopSessionAuthority).toHaveBeenCalledWith(
      "desktop-main",
      "session-9",
    );
    expect(session.connection.dispose).toHaveBeenCalledTimes(1);
    expect(port.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "terminalError",
    });
    expect(consoleError).toHaveBeenCalledWith(
      "stopped restoring persistent daemon subscriptions after terminal failure",
      staleError,
    );

    await vi.advanceTimersByTimeAsync(500);
    expect(session.connection.request).toHaveBeenCalledTimes(2);
  });

  it("stops retrying restore after daemon.initialize times out", async () => {
    vi.useFakeTimers();
    const session = new SessionStreamConnection("session-1") as any;
    const port = createFakePort();
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const socket = createScriptedRpcSocket(session.connection, [null]);

    session.runPorts.add(port);
    session.connection.dispose = vi.fn();
    session.ensureConnected = vi.fn(async () => {
      await session.connection.initializeConnection();
    });

    session.handleTransportTermination(new DaemonRpcUnavailableError("socket closed"));
    await vi.advanceTimersByTimeAsync(250);
    await vi.advanceTimersByTimeAsync(DAEMON_REQUEST_TIMEOUT_MS);

    expect(socket.write).toHaveBeenCalledTimes(1);
    expect(session.connection.dispose).toHaveBeenCalledTimes(1);
    expect(port.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "terminalError",
    });
    expect(consoleError).toHaveBeenCalledWith(
      "stopped restoring persistent daemon subscriptions after terminal failure",
      expect.any(DaemonRequestTimeoutError),
    );

    await vi.advanceTimersByTimeAsync(500);
    expect(socket.write).toHaveBeenCalledTimes(1);
  });

  it("stops retrying restore after daemon.session.attach times out", async () => {
    vi.useFakeTimers();
    const session = new SessionStreamConnection("session-1") as any;
    const port = createFakePort();
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const socket = createScriptedRpcSocket(session.connection, [
      { result: initializeResult("daemon-2") },
      null,
    ]);

    session.runPorts.add(port);
    session.connection.dispose = vi.fn();
    session.ensureConnected = vi.fn(async () => {
      await session.connection.initializeConnection();
    });

    session.handleTransportTermination(new DaemonRpcUnavailableError("socket closed"));
    await vi.advanceTimersByTimeAsync(250);
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(DAEMON_REQUEST_TIMEOUT_MS);

    expect(socket.write).toHaveBeenCalledTimes(2);
    expect(session.connection.dispose).toHaveBeenCalledTimes(1);
    expect(port.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "terminalError",
    });
    expect(consoleError).toHaveBeenCalledWith(
      "stopped restoring persistent daemon subscriptions after terminal failure",
      expect.any(DaemonRequestTimeoutError),
    );
  });

  it("stops retrying restore after daemon.subscribe times out", async () => {
    vi.useFakeTimers();
    const session = new SessionStreamConnection("session-1") as any;
    const port = createFakePort();
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const socket = createScriptedRpcSocket(session.connection, [
      { result: initializeResult("daemon-2") },
      { result: attachResult("session-1", "daemon-2") },
      null,
    ]);

    session.runPorts.add(port);
    session.connection.dispose = vi.fn();
    session.ensureConnected = vi.fn(async () => {
      await session.connection.initializeConnection();
    });

    session.handleTransportTermination(new DaemonRpcUnavailableError("socket closed"));
    await vi.advanceTimersByTimeAsync(250);
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(DAEMON_REQUEST_TIMEOUT_MS);

    expect(socket.write).toHaveBeenCalledTimes(3);
    expect(session.latestCursor).toEqual(resumeCursor(11, "session-1", "daemon-2"));
    expect(session.connection.dispose).toHaveBeenCalledTimes(1);
    expect(port.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "terminalError",
    });
    expect(consoleError).toHaveBeenCalledWith(
      "stopped restoring persistent daemon subscriptions after terminal failure",
      expect.any(DaemonRequestTimeoutError),
    );
  });

  it("does not schedule reconnect after a terminal transport failure", () => {
    const session = new SessionStreamConnection("session-1") as any;
    const port = createFakePort();

    session.runPorts.add(port);
    session.scheduleRestoreSubscriptions = vi.fn();
    session.connection.dispose = vi.fn();
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    session.handleTransportTermination(new DaemonProtocolError("daemon protocol mismatch"));

    expect(session.scheduleRestoreSubscriptions).not.toHaveBeenCalled();
    expect(session.connection.dispose).toHaveBeenCalledTimes(1);
    expect(port.postMessage).toHaveBeenCalledWith({
      stream: "runs",
      status: "terminalError",
    });
    expect(consoleError).toHaveBeenCalledWith(
      "stopped persistent daemon subscriptions after terminal failure",
      expect.any(DaemonProtocolError),
    );
  });

  it("schedules reconnect when transport dies during port attachment", async () => {
    const session = new SessionStreamConnection("session-1") as any;
    const port = createFakePort();
    const subscribeDeferred = createDeferred<void>();

    session.ensureSubscribedKinds = vi.fn(() => subscribeDeferred.promise);
    session.scheduleRestoreSubscriptions = vi.fn();

    const attachPromise = session.attachRunPort(port);
    await Promise.resolve();
    session.handleTransportTermination(new DaemonRpcUnavailableError("socket closed"));

    expect(session.scheduleRestoreSubscriptions).toHaveBeenCalledTimes(1);
    subscribeDeferred.reject(new DaemonRpcUnavailableError("socket closed"));
    await expect(attachPromise).rejects.toThrow("socket closed");
  });

  it("clears the replay cursor when the daemon epoch changes", () => {
    const session = new SessionStreamConnection("session-1") as any;

    session.daemonInstanceId = "daemon-1";
    session.latestCursor = resumeCursor(9);

    session.handleDaemonEpochId("daemon-2");

    expect(session.daemonInstanceId).toBe("daemon-2");
    expect(session.latestCursor).toBeNull();
  });

  it("ignores cursor updates from a different attached session", () => {
    const session = new SessionStreamConnection("session-9") as any;

    session.daemonInstanceId = "daemon-2";
    session.latestCursor = resumeCursor(9, "session-9", "daemon-2");

    session.noteLatestCursor(resumeCursor(11, "session-7", "daemon-2"));

    expect(session.latestCursor).toEqual(resumeCursor(9, "session-9", "daemon-2"));
  });

  it("initializes an attached stream connection and attaches the business session", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    const responses = [
      initializeResult("daemon-2"),
      {
        session: {
          id: "session-9",
          title: "Build daemon app server",
          status: "running",
        },
        sessionAuthority: "session-authority-2session-authority-2",
        latestCursor: {
          daemonInstanceId: "daemon-2",
          sessionId: "session-9",
          sequence: "11",
        },
      },
    ];

    session.connection.request = vi.fn(
      async (_method: string, _params: unknown, parseResult: (value: unknown) => unknown) => {
        const next = responses.shift();
        if (_method === "daemon.initialize") {
          return next;
        }
        return parseResult(next);
      },
    );

    await session.connection.initializeConnection();

    expect(session.connection.request).toHaveBeenNthCalledWith(
      1,
      "daemon.initialize",
      expect.any(Object),
      expect.any(Function),
    );
    expect(session.connection.request).toHaveBeenNthCalledWith(
      2,
      "daemon.session.attach",
      {
        sessionId: "session-9",
        sessionAuthority: "session-authority-1session-authority-1",
      },
      expect.any(Function),
    );
    expect(session.latestCursor).toEqual(resumeCursor(11, "session-9", "daemon-2"));
    expect(authorityStore.storeDesktopSessionAuthority).toHaveBeenCalledWith(
      "desktop-main",
      "session-9",
      "session-authority-2session-authority-2",
    );
  });

  it("purges local session authorities before attach when daemon.initialize rotates the client credential", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    credentialStore.loadDesktopClientCredential.mockResolvedValueOnce(
      "credential-oldcredential-oldcredential-old",
    );
    authorityStore.loadDesktopSessionAuthority.mockResolvedValueOnce(null);
    session.connection.request = vi.fn(
      async (method: string, _params: unknown, parseResult: (value: unknown) => unknown) => {
        if (method === "daemon.initialize") {
          return parseResult({
            ...initializeResult("daemon-2"),
            clientCredential: "credential-newcredential-newcredential-new",
          });
        }
        throw new Error("daemon.session.attach must not be called after credential rotation purge");
      },
    );

    await expect(session.connection.initializeConnection()).rejects.toThrow(
      new DaemonProtocolError("missing local session authority for session-9"),
    );

    expect(authorityStore.removeDesktopClientSessionAuthorities).toHaveBeenCalledWith(
      "desktop-main",
    );
    expect(credentialStore.storeDesktopClientCredential).toHaveBeenCalledWith(
      "desktop-main",
      "credential-newcredential-newcredential-new",
    );
    expect(session.connection.request).toHaveBeenCalledTimes(1);
  });

  it("rejects daemon.initialize protocol mismatch before mutating local credential or authority state", async () => {
    const session = new SessionStreamConnection("session-9") as any;
    credentialStore.loadDesktopClientCredential.mockResolvedValueOnce(
      "credential-oldcredential-oldcredential-old",
    );
    authorityStore.loadDesktopSessionAuthority.mockResolvedValueOnce(
      "session-authority-1session-authority-1",
    );
    session.connection.request = vi.fn(
      async (method: string, _params: unknown, parseResult: (value: unknown) => unknown) => {
        if (method !== "daemon.initialize") {
          throw new Error(`unexpected method after protocol mismatch: ${method}`);
        }
        return parseResult({
          ...initializeResult("daemon-2"),
          clientCredential: "credential-newcredential-newcredential-new",
          protocolVersion: "2026-04-wrong",
        });
      },
    );

    await expect(session.connection.initializeConnection()).rejects.toThrow(
      new DaemonProtocolError(
        "daemon protocol mismatch: expected 2026-04-stage3, got 2026-04-wrong",
      ),
    );

    expect(authorityStore.removeDesktopClientSessionAuthorities).not.toHaveBeenCalled();
    expect(credentialStore.storeDesktopClientCredential).not.toHaveBeenCalled();
    expect(authorityStore.loadDesktopSessionAuthority).not.toHaveBeenCalled();
    expect(session.connection.request).toHaveBeenCalledTimes(1);
  });

  it("initializes the request client before reads", async () => {
    const session = new DaemonSessionRequestClient() as any;
    const response = initializeResult("daemon-2");

    session.connection.request = vi.fn(async () => response);

    await session.connection.initializeConnection();

    expect(session.connection.request).toHaveBeenCalledWith(
      "daemon.initialize",
      {
        clientName: "desktop-main",
        clientCredential: null,
        clientVersion: "0.0.1",
        protocolVersion: "2026-04-stage3",
        capabilities: {
          notifications: true,
          eventSubscriptions: true,
        },
      },
      expect.any(Function),
    );
  });

  it("lists sessions through the daemon request client path", async () => {
    const session = new DaemonSessionRequestClient() as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi
      .fn()
      .mockResolvedValue([
        { id: "session-1", title: "Build daemon app server", status: "running" },
      ]);

    await expect(session.listSessions()).resolves.toEqual([
      { id: "session-1", title: "Build daemon app server", status: "running" },
    ]);
    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.session.list",
      {},
      expect.any(Function),
    );
  });

  it("lists only sessions with local desktop authority", async () => {
    const listSessions = vi.spyOn(DaemonSessionRequestClient.prototype, "listSessions");
    listSessions.mockResolvedValueOnce([
      { id: "session-1", title: "Attachable", status: "running" },
      { id: "session-2", title: "Orphaned", status: "idle" },
    ]);
    authorityStore.loadDesktopSessionAuthority
      .mockResolvedValueOnce("session-authority-1session-authority-1")
      .mockResolvedValueOnce(null);

    await expect(listDaemonSessions()).resolves.toEqual([
      { id: "session-1", title: "Attachable", status: "running" },
    ]);
    expect(authorityStore.loadDesktopSessionAuthority).toHaveBeenNthCalledWith(
      1,
      "desktop-main",
      "session-1",
    );
    expect(authorityStore.loadDesktopSessionAuthority).toHaveBeenNthCalledWith(
      2,
      "desktop-main",
      "session-2",
    );
  });

  it("requests the daemon-owned session overview through the canonical request client", async () => {
    const session = new DaemonSessionRequestClient() as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi.fn().mockResolvedValue({
      sessions: [
        {
          session: { id: "session-1", title: "Attachable", status: "running" },
          latestRun: {
            id: "run-1",
            objective: "Build daemon app server",
            status: "waitingForApproval",
          },
          laneStatus: "waitingForApproval",
          isActive: true,
          approvalAttention: "pending",
          pendingApprovalCount: 1,
          lastActivityAtMs: 42n,
          lastEventPreview: "Approval requested: execute run",
          recentActivity: [],
        },
      ],
    });

    await expect(session.getSessionOverview({ recentActivityLimit: 5 })).resolves.toEqual({
      sessions: [
        {
          session: { id: "session-1", title: "Attachable", status: "running" },
          latestRun: {
            id: "run-1",
            objective: "Build daemon app server",
            status: "waitingForApproval",
          },
          laneStatus: "waitingForApproval",
          isActive: true,
          approvalAttention: "pending",
          pendingApprovalCount: 1,
          lastActivityAtMs: 42n,
          lastEventPreview: "Approval requested: execute run",
          recentActivity: [],
        },
      ],
    });
    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.session.overview",
      { recentActivityLimit: 5 },
      expect.any(Function),
    );
  });

  it("does not invent a desktop-owned default session overview query", async () => {
    const session = new DaemonSessionRequestClient() as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi.fn();

    await expect(session.getSessionOverview(undefined)).rejects.toThrow(
      new ProtocolValidationError(
        "SessionOverviewQuery failed protocol validation: / must be object",
      ),
    );
    expect(session.sendRequest).not.toHaveBeenCalled();
  });

  it("filters session overview reads to sessions with local desktop authority", async () => {
    const getSessionOverview = vi.spyOn(DaemonSessionRequestClient.prototype, "getSessionOverview");
    getSessionOverview.mockResolvedValueOnce({
      sessions: [
        {
          session: { id: "session-1", title: "Attachable", status: "running" },
          latestRun: null,
          laneStatus: "idle",
          isActive: false,
          approvalAttention: "idle",
          pendingApprovalCount: 0,
          lastActivityAtMs: null,
          lastEventPreview: null,
          recentActivity: [],
        },
        {
          session: { id: "session-2", title: "Orphaned", status: "idle" },
          latestRun: null,
          laneStatus: "idle",
          isActive: false,
          approvalAttention: "idle",
          pendingApprovalCount: 0,
          lastActivityAtMs: null,
          lastEventPreview: null,
          recentActivity: [],
        },
      ],
    });
    authorityStore.loadDesktopSessionAuthority
      .mockResolvedValueOnce("session-authority-1session-authority-1")
      .mockResolvedValueOnce(null);

    await expect(getDaemonSessionOverview({ recentActivityLimit: 3 })).resolves.toEqual({
      sessions: [
        {
          session: { id: "session-1", title: "Attachable", status: "running" },
          latestRun: null,
          laneStatus: "idle",
          isActive: false,
          approvalAttention: "idle",
          pendingApprovalCount: 0,
          lastActivityAtMs: null,
          lastEventPreview: null,
          recentActivity: [],
        },
      ],
    });
    expect(getSessionOverview).toHaveBeenCalledWith({ recentActivityLimit: 3 });
    expect(authorityStore.loadDesktopSessionAuthority).toHaveBeenNthCalledWith(
      1,
      "desktop-main",
      "session-1",
    );
    expect(authorityStore.loadDesktopSessionAuthority).toHaveBeenNthCalledWith(
      2,
      "desktop-main",
      "session-2",
    );
  });

  it("opens a daemon-owned session through the daemon request client path", async () => {
    const session = new DaemonSessionRequestClient() as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi.fn().mockResolvedValue({
      session: { id: "session-1", title: "Build daemon app server", status: "idle" },
      latestCursor: resumeCursor(14),
      sessionAuthority: "session-authority-1session-authority-1",
    });

    await expect(session.openSession("Build daemon app server")).resolves.toEqual({
      id: "session-1",
      title: "Build daemon app server",
      status: "idle",
    });
    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.session.open",
      { title: "Build daemon app server" },
      expect.any(Function),
    );
    expect(authorityStore.storeDesktopSessionAuthority).toHaveBeenCalledWith(
      "desktop-main",
      "session-1",
      "session-authority-1session-authority-1",
    );
  });

  it("starts a run through the attached daemon request client path", async () => {
    const session = new DaemonSessionRequestClient("session-1") as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi.fn().mockResolvedValue({
      id: "run-1",
      objective: "Ship app server hard cut",
      status: "waitingForApproval",
    });

    await expect(session.startRun({ objective: "Ship app server hard cut" })).resolves.toEqual({
      id: "run-1",
      objective: "Ship app server hard cut",
      status: "waitingForApproval",
    });
    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.run.start",
      { objective: "Ship app server hard cut" },
      expect.any(Function),
    );
  });

  it("gets a single session through the daemon request client path", async () => {
    const session = new DaemonSessionRequestClient("session-1") as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi.fn().mockResolvedValue({
      id: "session-1",
      title: "Build daemon app server",
      status: "running",
    });

    await expect(session.getSession()).resolves.toEqual({
      id: "session-1",
      title: "Build daemon app server",
      status: "running",
    });
    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.session.get",
      {},
      expect.any(Function),
    );
  });

  it("lists approvals through the daemon request client path", async () => {
    const session = new DaemonSessionRequestClient("session-1") as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi.fn().mockResolvedValue({
      items: [{ id: "approval-1", runId: "run-1", scope: "processExec", reason: "need shell" }],
      latestCursor: {
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 9n,
      },
    });

    await expect(session.listApprovals({})).resolves.toEqual({
      items: [{ id: "approval-1", runId: "run-1", scope: "processExec", reason: "need shell" }],
      latestCursor: {
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 9n,
      },
    });
    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.approval.list",
      {},
      expect.any(Function),
    );
  });

  it("rejects malformed approval invoke args before reaching the daemon request client", async () => {
    const decideApproval = vi.spyOn(DaemonSessionRequestClient.prototype, "decideApproval");
    const invokeUnsafe = desktopSessionInvokeHandlers.decideApproval as (
      sessionId: unknown,
      approvalId: unknown,
      decision: unknown,
    ) => unknown;

    expect(() => invokeUnsafe("   ", "approval-1", "approved")).toThrow(
      /SessionId must be a non-empty string/,
    );
    expect(() => invokeUnsafe("session-1", {}, "approved")).toThrow(
      /ApprovalId failed protocol validation/,
    );
    expect(() => invokeUnsafe("session-1", "approval-1", "maybe")).toThrow(
      /ApprovalDecision failed protocol validation/,
    );

    expect(decideApproval).not.toHaveBeenCalled();
  });

  it("passes list-runs filters through the daemon request client path", async () => {
    const session = new DaemonSessionRequestClient("session-1") as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi
      .fn()
      .mockResolvedValue([
        { id: "run-1", objective: "Build daemon app server", status: "running" },
      ]);

    await expect(session.listRuns({})).resolves.toEqual([
      { id: "run-1", objective: "Build daemon app server", status: "running" },
    ]);
    expect(session.sendRequest).toHaveBeenCalledWith("daemon.run.list", {}, expect.any(Function));
  });

  it("rejects malformed list-approvals queries before reaching the daemon request client", async () => {
    const listApprovals = vi.spyOn(DaemonSessionRequestClient.prototype, "listApprovals");
    const invokeUnsafe = desktopSessionInvokeHandlers.listApprovals as (
      sessionId: unknown,
      query: unknown,
    ) => unknown;

    expect(() => invokeUnsafe("session-1", { runId: 7 })).toThrow(
      /ListApprovalsQuery failed protocol validation/,
    );

    expect(listApprovals).not.toHaveBeenCalled();
  });

  it("gets one activity page through the daemon request client path", async () => {
    const session = new DaemonSessionRequestClient("session-1") as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi.fn().mockResolvedValue({
      items: [
        {
          cursor: { sequence: 9n },
          occurredAtMs: 90n,
          event: {
            run: {
              runId: "run-1",
              status: "running",
              detail: "live",
            },
          },
        },
      ],
      nextBefore: { sequence: 9n },
      latestCursor: {
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 12n,
      },
    });

    await expect(session.getActivityPage({ limit: 25 })).resolves.toEqual({
      items: [
        {
          cursor: { sequence: 9n },
          occurredAtMs: 90n,
          event: {
            run: {
              runId: "run-1",
              status: "running",
              detail: "live",
            },
          },
        },
      ],
      nextBefore: { sequence: 9n },
      latestCursor: {
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 12n,
      },
    });
    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.activity.page",
      { limit: 25 },
      expect.any(Function),
    );
  });

  it("gets one committed agent turns page through the daemon request client path", async () => {
    const session = new DaemonSessionRequestClient("session-1") as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi.fn().mockResolvedValue({
      items: [
        {
          kind: "assistant",
          cursor: { sequence: 9n },
          sessionId: "session-1",
          runId: "run-1",
          turnId: "turn-1",
          startedAtMs: 90n,
          completedAtMs: 95n,
          text: "hello world",
        },
      ],
      nextBefore: { sequence: 9n },
      latestCursor: {
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 12n,
      },
    });

    await expect(session.getAgentTurnsPage({ limit: 25 })).resolves.toEqual({
      items: [
        {
          kind: "assistant",
          cursor: { sequence: 9n },
          sessionId: "session-1",
          runId: "run-1",
          turnId: "turn-1",
          startedAtMs: 90n,
          completedAtMs: 95n,
          text: "hello world",
        },
      ],
      nextBefore: { sequence: 9n },
      latestCursor: {
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 12n,
      },
    });
    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.agent.turns.page",
      { limit: 25 },
      expect.any(Function),
    );
  });

  it("passes list-artifacts filters through the daemon request client path", async () => {
    const session = new DaemonSessionRequestClient("session-1") as any;

    session.ensureConnected = vi.fn(async () => {});
    session.sendRequest = vi.fn().mockResolvedValue({
      items: [
        {
          id: "artifact-1",
          runId: "run-1",
          kind: "patch",
          storagePath: "artifacts/run-1/patch.diff",
        },
      ],
      latestCursor: {
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 7n,
      },
    });

    await expect(
      session.listArtifacts({ runId: "run-1", artifactId: "artifact-1" }),
    ).resolves.toEqual({
      items: [
        {
          id: "artifact-1",
          runId: "run-1",
          kind: "patch",
          storagePath: "artifacts/run-1/patch.diff",
        },
      ],
      latestCursor: {
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 7n,
      },
    });
    expect(session.sendRequest).toHaveBeenCalledWith(
      "daemon.artifact.list",
      { runId: "run-1", artifactId: "artifact-1" },
      expect.any(Function),
    );
  });

  it("times out a daemon-owned request client without wedging later requests", async () => {
    vi.useFakeTimers();
    const session = new DaemonSessionRequestClient() as any;
    const socket = createScriptedRpcSocket(session.connection, [
      { result: initializeResult("daemon-2") },
      null,
      {
        result: [{ id: "session-2", title: "Recovered daemon session", status: "idle" }],
      },
    ]);
    session.ensureConnected = vi.fn(async () => {
      if (session.connection.rpcConnection.initialized) {
        return;
      }
      await session.connection.initializeConnection();
      session.connection.rpcConnection.initialized = true;
    });

    const timedOut = expect(session.listSessions()).rejects.toBeInstanceOf(
      DaemonRequestTimeoutError,
    );
    await vi.advanceTimersByTimeAsync(DAEMON_REQUEST_TIMEOUT_MS);
    await timedOut;

    await expect(
      session.enqueueOperation(async () => {
        await session.ensureConnected();
        return session.sendRequest("daemon.session.list", {}, (value: unknown) => value);
      }),
    ).resolves.toEqual([{ id: "session-2", title: "Recovered daemon session", status: "idle" }]);
    expect(socket.write).toHaveBeenCalledTimes(3);
  });

  it("keeps attached request sessions separate from the stream session pool", async () => {
    const port = createFakePort();
    const disposedStreamSessions: SessionStreamConnection[] = [];
    const streamSessions: SessionStreamConnection[] = [];
    const requestSessions: DaemonSessionRequestClient[] = [];

    vi.spyOn(SessionStreamConnection.prototype as any, "ensureSubscribedKinds").mockImplementation(
      async function (this: SessionStreamConnection) {
        streamSessions.push(this);
      },
    );
    vi.spyOn(DaemonSessionRequestClient.prototype, "getSession").mockImplementation(
      async function (this: DaemonSessionRequestClient) {
        requestSessions.push(this);
        return null;
      },
    );
    const originalStreamDispose = Object.getOwnPropertyDescriptor(
      SessionStreamConnection.prototype,
      "dispose",
    )?.value as SessionStreamConnection["dispose"];
    vi.spyOn(SessionStreamConnection.prototype, "dispose").mockImplementation(
      function (this: SessionStreamConnection) {
        disposedStreamSessions.push(this);
        return Reflect.apply(originalStreamDispose, this, []);
      },
    );
    await attachRunStreamPort("session-1", port as never);
    await expect(getAttachedSession("session-1")).resolves.toBeNull();

    const streamSession = streamSessions.at(-1) ?? null;
    const requestSession = requestSessions.at(-1) ?? null;

    expect(streamSession).not.toBeNull();
    expect(requestSession).not.toBeNull();
    expect(requestSession).not.toBe(streamSession);
    expect(disposedStreamSessions).not.toContain(streamSession as SessionStreamConnection);
    expect((streamSession as any).runPorts.size).toBe(1);

    port.close();

    expect(disposedStreamSessions).toContain(streamSession as SessionStreamConnection);
  });

  it("reuses one pooled stream owner per session until the last port closes", async () => {
    const runPort = createFakePort();
    const approvalPort = createFakePort();
    const createdSessions: SessionStreamConnection[] = [];
    const disposedSessions: SessionStreamConnection[] = [];

    vi.spyOn(SessionStreamConnection.prototype as any, "ensureSubscribedKinds").mockImplementation(
      async function (this: SessionStreamConnection) {
        if (!createdSessions.includes(this)) {
          createdSessions.push(this);
        }
      },
    );
    const originalDispose = Object.getOwnPropertyDescriptor(
      SessionStreamConnection.prototype,
      "dispose",
    )?.value as SessionStreamConnection["dispose"];
    vi.spyOn(SessionStreamConnection.prototype, "dispose").mockImplementation(
      function (this: SessionStreamConnection) {
        disposedSessions.push(this);
        return Reflect.apply(originalDispose, this, []);
      },
    );

    await attachRunStreamPort("session-1", runPort as never);
    await attachApprovalStreamPort("session-1", approvalPort as never);

    expect(createdSessions).toHaveLength(1);
    expect(disposedSessions).toHaveLength(0);

    runPort.close();
    expect(disposedSessions).toHaveLength(0);

    approvalPort.close();
    expect(disposedSessions).toEqual([createdSessions[0] as SessionStreamConnection]);

    await attachRunStreamPort("session-1", createFakePort() as never);
    expect(createdSessions).toHaveLength(2);
  });

  it("reuses one pooled attached request session across concurrent run snapshot reads", async () => {
    const requestSessions: DaemonSessionRequestClient[] = [];

    vi.spyOn(DaemonSessionRequestClient.prototype, "listRuns").mockImplementation(
      async function (this: DaemonSessionRequestClient) {
        requestSessions.push(this);
        return [];
      },
    );
    vi.spyOn(DaemonSessionRequestClient.prototype, "getActivityPage").mockImplementation(
      async function (this: DaemonSessionRequestClient) {
        requestSessions.push(this);
        return {
          items: [],
          latestActivityCursor: null,
          nextBefore: null,
        };
      },
    );
    vi.spyOn(DaemonSessionRequestClient.prototype, "getAgentTurnsPage").mockImplementation(
      async function (this: DaemonSessionRequestClient) {
        requestSessions.push(this);
        return {
          items: [],
          latestCursor: null,
          nextBefore: null,
        };
      },
    );

    await Promise.all([
      listDaemonRuns("session-1"),
      desktopSessionInvokeHandlers.getActivityPage("session-1", { limit: 25 }),
      desktopSessionInvokeHandlers.getAgentTurnsPage("session-1", { limit: 25 }),
    ]);

    expect(requestSessions).toHaveLength(3);
    expect(requestSessions[0]).toBe(requestSessions[1]);
    expect(requestSessions[1]).toBe(requestSessions[2]);
  });

  it("purges local authority after an attached request hits terminal authority rejection", async () => {
    const disposedSessions: DaemonSessionRequestClient[] = [];
    const requestSessions: DaemonSessionRequestClient[] = [];
    const originalDispose = Object.getOwnPropertyDescriptor(
      DaemonSessionRequestClient.prototype,
      "dispose",
    )?.value as DaemonSessionRequestClient["dispose"];
    vi.spyOn(DaemonSessionRequestClient.prototype, "dispose").mockImplementation(
      function (this: DaemonSessionRequestClient) {
        disposedSessions.push(this);
        return Reflect.apply(originalDispose, this, []);
      },
    );
    vi.spyOn(DaemonSessionRequestClient.prototype, "getActivityPage").mockImplementationOnce(
      async function (this: DaemonSessionRequestClient) {
        requestSessions.push(this);
        throw new DaemonJsonRpcError(-32_602, "session authority rejected: session-1");
      },
    );
    vi.spyOn(DaemonSessionRequestClient.prototype, "getSession").mockImplementation(
      async function (this: DaemonSessionRequestClient) {
        requestSessions.push(this);
        return null;
      },
    );

    await expect(
      desktopSessionInvokeHandlers.getActivityPage("session-1", { limit: 25 }),
    ).rejects.toThrow();
    await getAttachedSession("session-1");

    expect(authorityStore.removeDesktopSessionAuthority).toHaveBeenCalledWith(
      "desktop-main",
      "session-1",
    );
    expect(disposedSessions).toHaveLength(1);
    expect(requestSessions).toHaveLength(2);
    expect(disposedSessions[0]).toBe(requestSessions[0]);
    expect(requestSessions[1]).not.toBe(requestSessions[0]);
  });

  it("keeps the pooled attached request session alive across timeouts", async () => {
    const requestSessions: DaemonSessionRequestClient[] = [];

    vi.spyOn(DaemonSessionRequestClient.prototype, "getSession").mockImplementation(
      async function (this: DaemonSessionRequestClient) {
        requestSessions.push(this);
        throw new DaemonRequestTimeoutError(
          `daemon request daemon.session.get timed out after ${DAEMON_REQUEST_TIMEOUT_MS}ms`,
        );
      },
    );
    await expect(getAttachedSession("session-1")).rejects.toBeInstanceOf(DaemonRequestTimeoutError);
    await expect(getAttachedSession("session-1")).rejects.toBeInstanceOf(DaemonRequestTimeoutError);

    expect(requestSessions).toHaveLength(2);
    expect(requestSessions[0]).toBe(requestSessions[1]);
  });

  it("fans out live events only to matching ports and deduplicates older sequences", () => {
    const session = new SessionStreamConnection("session-1") as any;
    const runPort = createFakePort();
    const approvalPort = createFakePort();
    const artifactPort = createFakePort();

    session.daemonInstanceId = "daemon-1";
    session.runPorts.add(runPort);
    session.approvalPorts.add(approvalPort);
    session.artifactPorts.add(artifactPort);

    session.handleDaemonEventEnvelope(runEvent(7, "live run"));

    expect(runPort.postMessage).toHaveBeenCalledTimes(1);
    expect(runPort.postMessage).toHaveBeenCalledWith(runEvent(7, "live run"));
    expect(approvalPort.postMessage).not.toHaveBeenCalled();
    expect(artifactPort.postMessage).not.toHaveBeenCalled();
    expect(session.latestCursor).toEqual(resumeCursor(7));

    session.handleDaemonEventEnvelope(runEvent(7, "duplicate"));
    session.handleDaemonEventEnvelope(runEvent(6, "older"));

    expect(runPort.postMessage).toHaveBeenCalledTimes(1);
    expect(session.latestCursor).toEqual(resumeCursor(7));
  });

  it("fans out approval envelopes with approval payload", () => {
    const session = new SessionStreamConnection("session-1") as any;
    const approvalPort = createFakePort();

    session.daemonInstanceId = "daemon-1";
    session.approvalPorts.add(approvalPort);

    session.handleDaemonEventEnvelope(approvalEvent(8, "need shell"));

    expect(approvalPort.postMessage).toHaveBeenCalledTimes(1);
    expect(approvalPort.postMessage).toHaveBeenCalledWith(approvalEvent(8, "need shell"));
    expect(session.latestCursor).toEqual(resumeCursor(8));
  });

  it("fans out artifact envelopes with artifact payload", () => {
    const session = new SessionStreamConnection("session-1") as any;
    const artifactPort = createFakePort();

    session.daemonInstanceId = "daemon-1";
    session.artifactPorts.add(artifactPort);

    session.handleDaemonEventEnvelope(artifactEvent(9, "run-9"));

    expect(artifactPort.postMessage).toHaveBeenCalledTimes(1);
    expect(artifactPort.postMessage).toHaveBeenCalledWith(artifactEvent(9, "run-9"));
    expect(session.latestCursor).toEqual(resumeCursor(9));
  });

  it("rejects daemon.event envelopes for a different attached session", () => {
    const session = new SessionStreamConnection("session-1") as any;
    const runPort = createFakePort();

    session.daemonInstanceId = "daemon-1";
    session.runPorts.add(runPort);

    expect(() =>
      session.handleDaemonEventEnvelope({
        ...runEvent(7, "live run"),
        sessionId: "session-other",
      }),
    ).toThrow(
      new DaemonProtocolError(
        "daemon.event returned envelope for wrong session: expected session-1, got session-other",
      ),
    );
    expect(runPort.postMessage).not.toHaveBeenCalled();
    expect(session.latestCursor).toBeNull();
  });

  it("rejects daemon.event envelopes for a different daemon instance", () => {
    const session = new SessionStreamConnection("session-1") as any;
    const runPort = createFakePort();

    session.daemonInstanceId = "daemon-1";
    session.runPorts.add(runPort);

    expect(() =>
      session.handleDaemonEventEnvelope({
        ...runEvent(7, "live run"),
        daemonInstanceId: "daemon-other",
      }),
    ).toThrow(
      new DaemonProtocolError(
        "daemon.event returned envelope for wrong daemon instance: expected daemon-1, got daemon-other",
      ),
    );
    expect(runPort.postMessage).not.toHaveBeenCalled();
    expect(session.latestCursor).toBeNull();
  });

  it("rejects daemon.event notifications before daemon.initialize completes", () => {
    const connection = new DaemonSessionConnection("session-1", {
      requestTimeout: DAEMON_REQUEST_TIMEOUT_DISABLED,
    }) as any;

    expect(() =>
      connection.rpcConnection.handleJsonRpcLine(
        JSON.stringify({
          jsonrpc: "2.0",
          method: METHOD_DAEMON_EVENT,
          params: {
            daemonInstanceId: "daemon-1",
            sessionId: "session-1",
            sequence: "7",
            occurredAtMs: "70",
            event: {
              run: {
                runId: "run-7",
                status: "queued",
                detail: "live run",
              },
            },
          },
        }),
      ),
    ).toThrow(new DaemonProtocolError("daemon.event arrived before daemon.initialize completed"));
  });
});

describe("daemon rpc transport", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("rejects an invalid deadline timeout policy at construction", () => {
    expect(
      () =>
        new DaemonRpcConnection({
          initializeConnection: async () => {},
          openSocket: () => ({ socket: {} as any, socketPath: "/tmp/taugentic.sock" }),
          requestTimeout: { kind: "deadline", ms: 0 },
        }),
    ).toThrow(new TypeError("daemon request timeout deadline must be a positive integer"));
  });

  it("rejects timed out requests with the dedicated timeout error and clears pending state", async () => {
    vi.useFakeTimers();
    const connection = new DaemonRpcConnection({
      initializeConnection: async () => {},
      openSocket: () => ({ socket: {} as any, socketPath: "/tmp/taugentic.sock" }),
      requestTimeout: { kind: "deadline", ms: 10 },
    }) as any;

    connection.socket = {
      destroyed: false,
      write: vi.fn((_line: string, callback?: (error?: Error | null) => void) => {
        callback?.(undefined);
        return true;
      }),
    };

    const request = expect(
      connection.request("daemon.subscribe", {}, (value: unknown) => value),
    ).rejects.toBeInstanceOf(DaemonRequestTimeoutError);
    await vi.advanceTimersByTimeAsync(10);

    await request;
    expect(connection.pendingRequests.size).toBe(0);
  });

  it("ignores a late response for a timed out request without terminating transport", async () => {
    vi.useFakeTimers();
    const onTransportTermination = vi.fn();
    const connection = new DaemonRpcConnection({
      initializeConnection: async () => {},
      openSocket: () => ({ socket: {} as any, socketPath: "/tmp/taugentic.sock" }),
      onTransportTermination,
      requestTimeout: { kind: "deadline", ms: 10 },
    }) as any;

    connection.socket = {
      destroyed: false,
      write: vi.fn((_line: string, callback?: (error?: Error | null) => void) => {
        callback?.(undefined);
        return true;
      }),
    };

    const timedOutRequest = expect(
      connection.request("daemon.subscribe", {}, (value: unknown) => value),
    ).rejects.toBeInstanceOf(DaemonRequestTimeoutError);
    await vi.advanceTimersByTimeAsync(10);
    await timedOutRequest;

    connection.handleSocketData(
      `${JSON.stringify({ jsonrpc: "2.0", id: 1, result: { status: "ready" } })}\n`,
    );

    const recoveredRequest = connection.request("daemon.subscribe", {}, (value: unknown) => value);
    connection.handleSocketData(
      `${JSON.stringify({ jsonrpc: "2.0", id: 2, result: { status: "ready" } })}\n`,
    );

    await expect(recoveredRequest).resolves.toEqual({ status: "ready" });
    expect(onTransportTermination).not.toHaveBeenCalled();
    expect(connection.pendingRequests.size).toBe(0);
    expect(connection.timedOutRequestIds.size).toBe(0);
  });

  it("clears pending timeout state after socket write failure", async () => {
    vi.useFakeTimers();
    const connection = new DaemonRpcConnection({
      initializeConnection: async () => {},
      openSocket: () => ({ socket: {} as any, socketPath: "/tmp/taugentic.sock" }),
      requestTimeout: { kind: "deadline", ms: 10 },
    }) as any;

    connection.socket = {
      destroyed: false,
      write: vi.fn((_line: string, callback?: (error?: Error | null) => void) => {
        callback?.(new Error("write failed"));
        return true;
      }),
    };

    const request = connection.request("daemon.subscribe", {}, (value: unknown) => value);
    await expect(request).rejects.toThrow("write failed");
    await vi.advanceTimersByTimeAsync(10);

    expect(connection.pendingRequests.size).toBe(0);
  });
});
