import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { DESKTOP_INVOKE_METHODS, DESKTOP_IPC_SCHEMA } from "../../packages/shared/src/ipc.js";

type IpcHandler = (...args: unknown[]) => unknown;
const testWorkspaceSelector = {
  kind: "byPath",
  path: "/tmp/taugentic-workspace",
  trustAcknowledged: false,
} as const;

const hoisted = vi.hoisted(() => {
  const registeredHandlers = new Map<string, IpcHandler>();

  return {
    ipcMain: {
      handle: vi.fn((channel: string, handler: IpcHandler) => {
        registeredHandlers.set(channel, handler);
      }),
    },
    registeredHandlers,
    startDaemonViaBootstrap: vi.fn(async () => ({ source: "bootstrap" })),
    readDaemonControlStateFromDaemon: vi.fn(async () => ({ source: "status" })),
    stopDaemonControlFromDaemon: vi.fn(async () => ({ source: "stop" })),
    enableDaemonBackgroundModeFromDaemon: vi.fn(async () => ({ source: "enable" })),
    disableDaemonBackgroundModeFromDaemon: vi.fn(async () => ({ source: "disable" })),
    reconcileDaemonControlFromDaemon: vi.fn(async () => ({ source: "reconcile" })),
    desktopAgentRuntimeInvokeHandlers: {
      getAgentRuntime: vi.fn(async () => ({ runtimeProfileId: "runtime-codex-safe" })),
      selectAgentRuntimeProfile: vi.fn(async (params: unknown) => ({ selected: params })),
      patchAgentRuntimeProfile: vi.fn(async (params: unknown) => ({ patched: params })),
      loginAgentRuntimeAuthProfile: vi.fn(async (params: unknown) => ({ login: params })),
      logoutAgentRuntimeAuthProfile: vi.fn(async (params: unknown) => ({ logout: params })),
      setAgentRuntimeExtensionEnabled: vi.fn(async (params: unknown) => ({ extension: params })),
    },
    desktopSessionInvokeHandlers: {
      listSessions: vi.fn(async () => [{ id: "session-1" }]),
      getSessionOverview: vi.fn(async (query: unknown) => ({ query })),
      openSession: vi.fn(async (title: string, workspace: unknown) => ({
        id: "session-1",
        title,
        workspace,
      })),
      listRecipes: vi.fn(async () => ({ recipes: [] })),
      listWorkItems: vi.fn(async () => ({ items: [], sync: { state: "idle" } })),
      loadWorkflow: vi.fn(async (params: unknown) => ({ loaded: params })),
      getWorkflowStatus: vi.fn(async () => ({ loaded: null })),
      reloadWorkflow: vi.fn(async () => ({ loaded: null })),
      validateWorkflow: vi.fn(async () => ({ valid: true, errors: [] })),
      refreshWorkItems: vi.fn(async () => ({ items: [], sync: { state: "refreshQueued" } })),
      dismissWorkItem: vi.fn(async (params: unknown) => ({ item: params })),
      triggerWorkItem: vi.fn(async (...args: unknown[]) => ({ args })),
      startRun: vi.fn(async (sessionId: string, objective: string) => ({ sessionId, objective })),
      forkRun: vi.fn(async (...args: unknown[]) => ({ args })),
      decideApproval: vi.fn(async (...args: unknown[]) => ({ args })),
      getSession: vi.fn(async (sessionId: string) => ({ id: sessionId })),
      getActivityPage: vi.fn(async (...args: unknown[]) => ({ args })),
      getAgentTurnsPage: vi.fn(async (...args: unknown[]) => ({ args })),
      listApprovals: vi.fn(async (...args: unknown[]) => ({ args })),
      listArtifacts: vi.fn(async (...args: unknown[]) => ({ args })),
      readArtifactContent: vi.fn(async (...args: unknown[]) => ({ args })),
      saveArtifactAs: vi.fn(async (...args: unknown[]) => ({ args })),
      getRunDetail: vi.fn(async (...args: unknown[]) => ({ args })),
      getRunTimeline: vi.fn(async (...args: unknown[]) => ({ args })),
      replayRunEvents: vi.fn(async (...args: unknown[]) => ({ args })),
      listRuns: vi.fn(async (sessionId: string) => [{ sessionId }]),
      listNativeRuns: vi.fn(async (...args: unknown[]) => ({ args })),
    },
  };
});

vi.mock("electron", () => ({
  ipcMain: hoisted.ipcMain,
}));

vi.mock("../../packages/main/src/daemon-bootstrap.js", () => ({
  startDaemonViaBootstrap: hoisted.startDaemonViaBootstrap,
}));

vi.mock("../../packages/main/src/daemon-rpc-client.js", () => ({
  readDaemonControlStateFromDaemon: hoisted.readDaemonControlStateFromDaemon,
  stopDaemonControlFromDaemon: hoisted.stopDaemonControlFromDaemon,
  enableDaemonBackgroundModeFromDaemon: hoisted.enableDaemonBackgroundModeFromDaemon,
  disableDaemonBackgroundModeFromDaemon: hoisted.disableDaemonBackgroundModeFromDaemon,
  reconcileDaemonControlFromDaemon: hoisted.reconcileDaemonControlFromDaemon,
}));

vi.mock("../../packages/main/src/daemon-session.js", () => ({
  desktopSessionInvokeHandlers: hoisted.desktopSessionInvokeHandlers,
  listDaemonNativeRuns: hoisted.desktopSessionInvokeHandlers.listNativeRuns,
  listDaemonRuns: hoisted.desktopSessionInvokeHandlers.listRuns,
  listDaemonSessions: hoisted.desktopSessionInvokeHandlers.listSessions,
}));

vi.mock("../../packages/main/src/daemon-agent-runtime.js", () => ({
  desktopAgentRuntimeInvokeHandlers: hoisted.desktopAgentRuntimeInvokeHandlers,
}));

describe("rpc", () => {
  beforeEach(() => {
    vi.resetModules();
    hoisted.ipcMain.handle.mockClear();
    hoisted.registeredHandlers.clear();
    hoisted.startDaemonViaBootstrap.mockClear();
    hoisted.readDaemonControlStateFromDaemon.mockClear();
    hoisted.stopDaemonControlFromDaemon.mockClear();
    hoisted.enableDaemonBackgroundModeFromDaemon.mockClear();
    hoisted.disableDaemonBackgroundModeFromDaemon.mockClear();
    hoisted.reconcileDaemonControlFromDaemon.mockClear();
    for (const handler of Object.values(hoisted.desktopAgentRuntimeInvokeHandlers)) {
      handler.mockClear();
    }
    for (const handler of Object.values(hoisted.desktopSessionInvokeHandlers)) {
      handler.mockClear();
    }
  });

  it("registers desktop invoke handlers exactly once", async () => {
    const { registerDesktopRpcHandlers } = await import("../../packages/main/src/rpc.js");

    registerDesktopRpcHandlers();
    registerDesktopRpcHandlers();

    expect(hoisted.ipcMain.handle).toHaveBeenCalledTimes(DESKTOP_INVOKE_METHODS.length);
    expect(hoisted.registeredHandlers.size).toBe(DESKTOP_INVOKE_METHODS.length);
  });

  it("dispatches registered channels to the canonical invoke handlers", async () => {
    const { registerDesktopRpcHandlers } = await import("../../packages/main/src/rpc.js");
    registerDesktopRpcHandlers();

    const startDaemonHandler = hoisted.registeredHandlers.get(
      DESKTOP_IPC_SCHEMA.startDaemon.channel,
    );
    const sessionOverviewHandler = hoisted.registeredHandlers.get(
      DESKTOP_IPC_SCHEMA.getSessionOverview.channel,
    );
    const getAgentRuntimeHandler = hoisted.registeredHandlers.get(
      DESKTOP_IPC_SCHEMA.getAgentRuntime.channel,
    );
    const patchAgentRuntimeProfileHandler = hoisted.registeredHandlers.get(
      DESKTOP_IPC_SCHEMA.patchAgentRuntimeProfile.channel,
    );
    const openSessionHandler = hoisted.registeredHandlers.get(
      DESKTOP_IPC_SCHEMA.openSession.channel,
    );

    await expect(startDaemonHandler?.({})).resolves.toEqual({ source: "bootstrap" });
    await expect(getAgentRuntimeHandler?.({})).resolves.toEqual({
      runtimeProfileId: "runtime-codex-safe",
    });
    await expect(
      patchAgentRuntimeProfileHandler?.(
        {},
        {
          runtimeProfileId: "runtime-codex-safe",
          patch: { policyMode: "allow" },
        },
      ),
    ).resolves.toEqual({
      patched: {
        runtimeProfileId: "runtime-codex-safe",
        patch: { policyMode: "allow" },
      },
    });
    await expect(sessionOverviewHandler?.({}, { recentActivityLimit: 5 })).resolves.toEqual({
      query: { recentActivityLimit: 5 },
    });
    await expect(openSessionHandler?.({}, "Fresh session", testWorkspaceSelector)).resolves.toEqual({
      id: "session-1",
      title: "Fresh session",
      workspace: testWorkspaceSelector,
    });

    expect(hoisted.startDaemonViaBootstrap).toHaveBeenCalledTimes(1);
    expect(hoisted.desktopAgentRuntimeInvokeHandlers.getAgentRuntime).toHaveBeenCalledTimes(1);
    expect(hoisted.desktopAgentRuntimeInvokeHandlers.patchAgentRuntimeProfile).toHaveBeenCalledWith(
      {
        runtimeProfileId: "runtime-codex-safe",
        patch: { policyMode: "allow" },
      },
    );
    expect(hoisted.desktopSessionInvokeHandlers.getSessionOverview).toHaveBeenCalledWith({
      recentActivityLimit: 5,
    });
    expect(hoisted.desktopSessionInvokeHandlers.openSession).toHaveBeenCalledWith(
      "Fresh session",
      testWorkspaceSelector,
    );
  });

  it("rejects invoke calls with extra positional args before reaching handlers", async () => {
    const { registerDesktopRpcHandlers } = await import("../../packages/main/src/rpc.js");
    registerDesktopRpcHandlers();

    const openSessionHandler = hoisted.registeredHandlers.get(
      DESKTOP_IPC_SCHEMA.openSession.channel,
    );
    if (!openSessionHandler) {
      throw new Error("expected openSession handler to be registered");
    }

    expect(() =>
      openSessionHandler({}, "Fresh session", testWorkspaceSelector, "extra arg"),
    ).toThrow(
      "desktop IPC method openSession expected 2 arg(s), got 3",
    );
    expect(hoisted.desktopSessionInvokeHandlers.openSession).not.toHaveBeenCalled();
  });
});
