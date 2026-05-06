import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import {
  createDesktopStreamOpenErrorResponse,
  createDesktopStreamOpenSuccessResponse,
  DESKTOP_IPC_SCHEMA,
  getDesktopStreamResponseChannel,
} from "../../packages/shared/src/ipc.js";

type StreamHandler = (event: Electron.IpcMainEvent, ...args: unknown[]) => void;

const hoisted = vi.hoisted(() => {
  const handlers = new Map<string, StreamHandler>();

  return {
    attachAgentStreamPort: vi.fn<
      (sessionId: string, port: unknown, afterCursor: unknown) => Promise<void>
    >(async () => {}),
    attachApprovalStreamPort: vi.fn<
      (sessionId: string, port: unknown, afterCursor: unknown) => Promise<void>
    >(async () => {}),
    attachArtifactStreamPort: vi.fn<
      (sessionId: string, port: unknown, afterCursor: unknown) => Promise<void>
    >(async () => {}),
    attachRunStreamPort: vi.fn<
      (sessionId: string, port: unknown, afterCursor: unknown) => Promise<void>
    >(async () => {}),
    attachRunEventStreamPort: vi.fn<
      (sessionId: string, runId: string, port: unknown, afterSeq: unknown) => Promise<void>
    >(async () => {}),
    handlers,
    ipcMain: {
      on: vi.fn((channel: string, handler: StreamHandler) => {
        handlers.set(channel, handler);
      }),
    },
    messageChannelFactory: vi.fn(() => ({
      port1: {
        close: vi.fn(),
      },
      port2: {
        name: "port-2",
      },
    })),
  };
});

vi.mock("electron", () => ({
  ipcMain: hoisted.ipcMain,
  MessageChannelMain: vi.fn(function MessageChannelMainMock() {
    return hoisted.messageChannelFactory();
  }),
}));

vi.mock("../../packages/main/src/daemon-session.js", () => ({
  attachAgentStreamPort: (sessionId: string, port: unknown, afterCursor: unknown) =>
    hoisted.attachAgentStreamPort(sessionId, port, afterCursor),
  attachApprovalStreamPort: (sessionId: string, port: unknown, afterCursor: unknown) =>
    hoisted.attachApprovalStreamPort(sessionId, port, afterCursor),
  attachArtifactStreamPort: (sessionId: string, port: unknown, afterCursor: unknown) =>
    hoisted.attachArtifactStreamPort(sessionId, port, afterCursor),
  attachRunStreamPort: (sessionId: string, port: unknown, afterCursor: unknown) =>
    hoisted.attachRunStreamPort(sessionId, port, afterCursor),
  attachRunEventStreamPort: (sessionId: string, runId: string, port: unknown, afterSeq: unknown) =>
    hoisted.attachRunEventStreamPort(sessionId, runId, port, afterSeq),
  desktopSessionStreamHandlers: {
    openRunEventStream: (sessionId: string, runId: string, port: unknown, afterSeq: unknown) =>
      hoisted.attachRunEventStreamPort(sessionId, runId, port, afterSeq),
    openAgentStream: (sessionId: string, port: unknown, afterCursor: unknown) =>
      hoisted.attachAgentStreamPort(sessionId, port, afterCursor),
    openRunStream: (sessionId: string, port: unknown, afterCursor: unknown) =>
      hoisted.attachRunStreamPort(sessionId, port, afterCursor),
    openApprovalStream: (sessionId: string, port: unknown, afterCursor: unknown) =>
      hoisted.attachApprovalStreamPort(sessionId, port, afterCursor),
    openArtifactStream: (sessionId: string, port: unknown, afterCursor: unknown) =>
      hoisted.attachArtifactStreamPort(sessionId, port, afterCursor),
  },
}));

describe("registerDesktopStreamHandlers", () => {
  beforeEach(() => {
    vi.resetModules();
    hoisted.handlers.clear();
    hoisted.ipcMain.on.mockClear();
    hoisted.attachAgentStreamPort.mockReset();
    hoisted.attachAgentStreamPort.mockResolvedValue(undefined);
    hoisted.attachApprovalStreamPort.mockReset();
    hoisted.attachApprovalStreamPort.mockResolvedValue(undefined);
    hoisted.attachArtifactStreamPort.mockReset();
    hoisted.attachArtifactStreamPort.mockResolvedValue(undefined);
    hoisted.attachRunStreamPort.mockReset();
    hoisted.attachRunStreamPort.mockResolvedValue(undefined);
    hoisted.attachRunEventStreamPort.mockReset();
    hoisted.attachRunEventStreamPort.mockResolvedValue(undefined);
    hoisted.messageChannelFactory.mockClear();
  });

  it("registers the schema stream channels once and forwards explicit session ids", async () => {
    const { registerDesktopStreamHandlers } = await import("../../packages/main/src/streams.js");

    registerDesktopStreamHandlers();
    registerDesktopStreamHandlers();

    expect(hoisted.ipcMain.on).toHaveBeenCalledTimes(5);

    const sender = { postMessage: vi.fn() };
    const runChannel = hoisted.handlers.get(DESKTOP_IPC_SCHEMA.openRunStream.requestChannel);
    const runEventChannel = hoisted.handlers.get(
      DESKTOP_IPC_SCHEMA.openRunEventStream.requestChannel,
    );
    const approvalChannel = hoisted.handlers.get(
      DESKTOP_IPC_SCHEMA.openApprovalStream.requestChannel,
    );
    const artifactChannel = hoisted.handlers.get(
      DESKTOP_IPC_SCHEMA.openArtifactStream.requestChannel,
    );
    const agentChannel = hoisted.handlers.get(DESKTOP_IPC_SCHEMA.openAgentStream.requestChannel);
    if (!runChannel || !runEventChannel || !approvalChannel || !artifactChannel || !agentChannel) {
      throw new Error("expected stream handlers to be registered");
    }
    const ipcEvent = { sender } as unknown as Electron.IpcMainEvent;

    runChannel(ipcEvent, {
      requestId: "run-request",
      args: ["session-run", null],
    });
    runEventChannel(ipcEvent, {
      requestId: "run-event-request",
      args: ["session-run-event", "run-1", 7n],
    });
    approvalChannel(ipcEvent, {
      requestId: "approval-request",
      args: ["session-approval", null],
    });
    artifactChannel(ipcEvent, {
      requestId: "artifact-request",
      args: ["session-artifact", null],
    });
    agentChannel(ipcEvent, {
      requestId: "agent-request",
      args: ["session-agent", null],
    });
    await Promise.resolve();

    expect(sender.postMessage).toHaveBeenNthCalledWith(
      1,
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, "run-request"),
      createDesktopStreamOpenSuccessResponse(),
      [expect.objectContaining({ name: "port-2" })],
    );
    expect(sender.postMessage).toHaveBeenNthCalledWith(
      2,
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunEventStream, "run-event-request"),
      createDesktopStreamOpenSuccessResponse(),
      [expect.objectContaining({ name: "port-2" })],
    );
    expect(sender.postMessage).toHaveBeenNthCalledWith(
      3,
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openApprovalStream, "approval-request"),
      createDesktopStreamOpenSuccessResponse(),
      [expect.objectContaining({ name: "port-2" })],
    );
    expect(sender.postMessage).toHaveBeenNthCalledWith(
      4,
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openArtifactStream, "artifact-request"),
      createDesktopStreamOpenSuccessResponse(),
      [expect.objectContaining({ name: "port-2" })],
    );
    expect(sender.postMessage).toHaveBeenNthCalledWith(
      5,
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openAgentStream, "agent-request"),
      createDesktopStreamOpenSuccessResponse(),
      [expect.objectContaining({ name: "port-2" })],
    );
    expect(hoisted.attachRunStreamPort).toHaveBeenCalledWith(
      "session-run",
      expect.objectContaining({ close: expect.any(Function) }),
      null,
    );
    expect(hoisted.attachApprovalStreamPort).toHaveBeenCalledWith(
      "session-approval",
      expect.objectContaining({ close: expect.any(Function) }),
      null,
    );
    expect(hoisted.attachArtifactStreamPort).toHaveBeenCalledWith(
      "session-artifact",
      expect.objectContaining({ close: expect.any(Function) }),
      null,
    );
    expect(hoisted.attachAgentStreamPort).toHaveBeenCalledWith(
      "session-agent",
      expect.objectContaining({ close: expect.any(Function) }),
      null,
    );
    expect(hoisted.attachRunEventStreamPort).toHaveBeenCalledWith(
      "session-run-event",
      "run-1",
      expect.objectContaining({ close: expect.any(Function) }),
      7n,
    );
  });

  it("parses daemon event cursors for agent stream opens before attaching", async () => {
    const { registerDesktopStreamHandlers } = await import("../../packages/main/src/streams.js");

    registerDesktopStreamHandlers();

    const sender = { postMessage: vi.fn() };
    const agentChannel = hoisted.handlers.get(DESKTOP_IPC_SCHEMA.openAgentStream.requestChannel);
    if (!agentChannel) {
      throw new Error("expected agent stream handler to be registered");
    }
    const ipcEvent = { sender } as unknown as Electron.IpcMainEvent;

    agentChannel(ipcEvent, {
      requestId: "agent-request",
      args: [
        "session-agent",
        {
          daemonInstanceId: "daemon-7",
          sessionId: "session-agent",
          sequence: "19",
        },
      ],
    });
    await Promise.resolve();

    expect(hoisted.attachAgentStreamPort).toHaveBeenCalledWith(
      "session-agent",
      expect.objectContaining({ close: expect.any(Function) }),
      {
        daemonInstanceId: "daemon-7",
        sessionId: "session-agent",
        sequence: 19n,
      },
    );
  });

  it("fails fast when the renderer does not provide a session id", async () => {
    const { registerDesktopStreamHandlers } = await import("../../packages/main/src/streams.js");

    registerDesktopStreamHandlers();

    const sender = { postMessage: vi.fn() };
    const runChannel = hoisted.handlers.get(DESKTOP_IPC_SCHEMA.openRunStream.requestChannel);
    if (!runChannel) {
      throw new Error("expected run stream handler to be registered");
    }
    const ipcEvent = { sender } as unknown as Electron.IpcMainEvent;

    expect(() =>
      runChannel(ipcEvent, {
        requestId: "run-request",
        args: ["", null],
      }),
    ).toThrow("SessionId must be a non-empty string");
    expect(sender.postMessage).not.toHaveBeenCalled();
    expect(hoisted.attachRunStreamPort).not.toHaveBeenCalled();
  });

  it("rejects stream requests with extra positional args before opening a port", async () => {
    const { registerDesktopStreamHandlers } = await import("../../packages/main/src/streams.js");

    registerDesktopStreamHandlers();

    const sender = { postMessage: vi.fn() };
    const runChannel = hoisted.handlers.get(DESKTOP_IPC_SCHEMA.openRunStream.requestChannel);
    if (!runChannel) {
      throw new Error("expected run stream handler to be registered");
    }
    const ipcEvent = { sender } as unknown as Electron.IpcMainEvent;

    expect(() =>
      runChannel(ipcEvent, { requestId: "run-request", args: ["session-run", null] }, "extra arg"),
    ).toThrow("desktop stream request openRunStream expected 1 payload arg, got 2");
    expect(sender.postMessage).not.toHaveBeenCalled();
    expect(hoisted.attachRunStreamPort).not.toHaveBeenCalled();
  });

  it("rejects stream open before transferring a port when session attachment fails", async () => {
    const attachError = new Error("attach failed");
    hoisted.attachRunStreamPort.mockRejectedValueOnce(attachError);
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const { registerDesktopStreamHandlers } = await import("../../packages/main/src/streams.js");

    registerDesktopStreamHandlers();

    const sender = { postMessage: vi.fn() };
    const runChannel = hoisted.handlers.get(DESKTOP_IPC_SCHEMA.openRunStream.requestChannel);
    if (!runChannel) {
      throw new Error("expected run stream handler to be registered");
    }
    const ipcEvent = { sender } as unknown as Electron.IpcMainEvent;

    runChannel(ipcEvent, {
      requestId: "run-request",
      args: ["session-run", null],
    });
    await Promise.resolve();
    await Promise.resolve();

    const firstPort = hoisted.messageChannelFactory.mock.results[0]?.value.port1;
    expect(firstPort?.close).toHaveBeenCalledTimes(1);
    expect(sender.postMessage).toHaveBeenCalledWith(
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, "run-request"),
      createDesktopStreamOpenErrorResponse("attach failed"),
    );
    expect(consoleError).toHaveBeenCalledWith(
      "failed to attach openRunStream to daemon session",
      attachError,
    );
  });

  it("does not post a success reply until session attachment completes", async () => {
    const attachDeferred = Promise.withResolvers<void>();
    hoisted.attachRunStreamPort.mockReturnValueOnce(attachDeferred.promise);
    const { registerDesktopStreamHandlers } = await import("../../packages/main/src/streams.js");

    registerDesktopStreamHandlers();

    const sender = { postMessage: vi.fn() };
    const runChannel = hoisted.handlers.get(DESKTOP_IPC_SCHEMA.openRunStream.requestChannel);
    if (!runChannel) {
      throw new Error("expected run stream handler to be registered");
    }
    const ipcEvent = { sender } as unknown as Electron.IpcMainEvent;

    runChannel(ipcEvent, {
      requestId: "run-request",
      args: ["session-run", null],
    });
    await Promise.resolve();

    expect(sender.postMessage).not.toHaveBeenCalled();

    attachDeferred.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(sender.postMessage).toHaveBeenCalledWith(
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, "run-request"),
      createDesktopStreamOpenSuccessResponse(),
      [expect.objectContaining({ name: "port-2" })],
    );
  });

  it("echoes distinct request ids for concurrent same-kind stream opens", async () => {
    const { registerDesktopStreamHandlers } = await import("../../packages/main/src/streams.js");

    registerDesktopStreamHandlers();

    const sender = { postMessage: vi.fn() };
    const runChannel = hoisted.handlers.get(DESKTOP_IPC_SCHEMA.openRunStream.requestChannel);
    if (!runChannel) {
      throw new Error("expected run stream handler to be registered");
    }
    const ipcEvent = { sender } as unknown as Electron.IpcMainEvent;

    runChannel(ipcEvent, {
      requestId: "run-request-1",
      args: ["session-run-1", null],
    });
    runChannel(ipcEvent, {
      requestId: "run-request-2",
      args: ["session-run-2", null],
    });
    await Promise.resolve();

    expect(sender.postMessage).toHaveBeenNthCalledWith(
      1,
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, "run-request-1"),
      createDesktopStreamOpenSuccessResponse(),
      [expect.objectContaining({ name: "port-2" })],
    );
    expect(sender.postMessage).toHaveBeenNthCalledWith(
      2,
      getDesktopStreamResponseChannel(DESKTOP_IPC_SCHEMA.openRunStream, "run-request-2"),
      createDesktopStreamOpenSuccessResponse(),
      [expect.objectContaining({ name: "port-2" })],
    );
    expect(hoisted.attachRunStreamPort).toHaveBeenNthCalledWith(
      1,
      "session-run-1",
      expect.objectContaining({ close: expect.any(Function) }),
      null,
    );
    expect(hoisted.attachRunStreamPort).toHaveBeenNthCalledWith(
      2,
      "session-run-2",
      expect.objectContaining({ close: expect.any(Function) }),
      null,
    );
  });

  it("forwards the run snapshot cursor to the run stream owner", async () => {
    const { registerDesktopStreamHandlers } = await import("../../packages/main/src/streams.js");

    registerDesktopStreamHandlers();

    const sender = { postMessage: vi.fn() };
    const runChannel = hoisted.handlers.get(DESKTOP_IPC_SCHEMA.openRunStream.requestChannel);
    if (!runChannel) {
      throw new Error("expected run stream handler to be registered");
    }
    const ipcEvent = { sender } as unknown as Electron.IpcMainEvent;

    runChannel(ipcEvent, {
      requestId: "run-request",
      args: ["session-run", { sequence: "12" }],
    });
    await Promise.resolve();

    expect(hoisted.attachRunStreamPort).toHaveBeenCalledWith(
      "session-run",
      expect.objectContaining({ close: expect.any(Function) }),
      { sequence: 12n },
    );
  });
});
