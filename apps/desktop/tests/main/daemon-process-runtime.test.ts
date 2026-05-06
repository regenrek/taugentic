import type { DaemonControlStatusResult } from "../../packages/shared/src/contracts.js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { DaemonRpcUnavailableError } from "../../packages/main/src/daemon-rpc-connection.js";

function createControlState(
  overrides: Partial<DaemonControlStatusResult> = {},
): DaemonControlStatusResult {
  return {
    actualMode: "stopped",
    allowedActions: ["start", "enableBackground"],
    backgroundOptIn: false,
    desiredMode: "local",
    errorCode: null,
    logPath: "/tmp/taugentic-test.log",
    message: "Local mode is the desired runtime.",
    daemonVersion: null,
    pendingTransition: null,
    protocolVersion: "2026-04-stage2",
    reconcileRequired: false,
    socketPath: "/tmp/taugentic-test.sock",
    transitionStatus: "idle",
    ...overrides,
  };
}

const hoisted = vi.hoisted(() => ({
  controlState: createControlState(),
  mockReadDaemonControlState: vi.fn(async () => hoisted.controlState),
  mockReconcileDaemonControl: vi.fn(async () => {
    hoisted.controlState = createControlState({
      allowedActions: ["stop"],
      actualMode: "local",
      desiredMode: "local",
      message: "Local mode is the desired runtime.",
      daemonVersion: "0.0.1-test",
    });
    return hoisted.controlState;
  }),
  mockStartConfiguredDaemon: vi.fn(async () => {
    hoisted.controlState = createControlState({
      allowedActions: ["stop"],
      actualMode: "local",
      desiredMode: "local",
      message: "Local mode is the desired runtime.",
      daemonVersion: "0.0.1-test",
    });
    return hoisted.controlState;
  }),
  mockStopConfiguredDaemon: vi.fn(async () => {
    hoisted.controlState = createControlState({
      actualMode: "stopped",
      allowedActions:
        hoisted.controlState.desiredMode === "background"
          ? ["start", "disableBackground"]
          : ["start", "enableBackground"],
      backgroundOptIn: hoisted.controlState.desiredMode === "background",
      desiredMode: hoisted.controlState.desiredMode,
    });
    return hoisted.controlState;
  }),
}));

vi.mock("../../packages/main/src/daemon-rpc-client.js", () => ({
  readDaemonControlStateFromDaemon: () => hoisted.mockReadDaemonControlState(),
  reconcileDaemonControlFromDaemon: () => hoisted.mockReconcileDaemonControl(),
  stopDaemonControlFromDaemon: () => hoisted.mockStopConfiguredDaemon(),
  disableDaemonBackgroundModeFromDaemon: vi.fn(),
  enableDaemonBackgroundModeFromDaemon: vi.fn(),
}));

vi.mock("../../packages/main/src/daemon-bootstrap.js", () => ({
  startDaemonViaBootstrap: () => hoisted.mockStartConfiguredDaemon(),
}));

describe("desktop daemon runtime surface", () => {
  beforeEach(() => {
    vi.resetModules();
    hoisted.controlState = createControlState();
    hoisted.mockReadDaemonControlState.mockClear();
    hoisted.mockReconcileDaemonControl.mockClear();
    hoisted.mockStartConfiguredDaemon.mockClear();
    hoisted.mockStopConfiguredDaemon.mockClear();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("starts the configured runtime when Rust exposes only the start action", async () => {
    const { ensureDesktopDaemonRuntime } =
      await import("../../packages/main/src/daemon-process.js");
    await ensureDesktopDaemonRuntime();

    expect(hoisted.mockReadDaemonControlState).toHaveBeenCalledTimes(1);
    expect(hoisted.mockStartConfiguredDaemon).toHaveBeenCalledTimes(1);
    expect(hoisted.mockReconcileDaemonControl).not.toHaveBeenCalled();
  });

  it("prefers reconcile when Rust exposes only the reconcile action", async () => {
    hoisted.controlState = createControlState({
      allowedActions: ["reconcile"],
      errorCode: "transitionFailed",
      message: "Reconcile required.",
      reconcileRequired: true,
      transitionStatus: "degradedReconcileRequired",
    });

    const { ensureDesktopDaemonRuntime } =
      await import("../../packages/main/src/daemon-process.js");
    await ensureDesktopDaemonRuntime();

    expect(hoisted.mockReconcileDaemonControl).toHaveBeenCalledTimes(1);
    expect(hoisted.mockStartConfiguredDaemon).not.toHaveBeenCalled();
  });

  it("starts via bootstrap when control-state RPC is unavailable during startup", async () => {
    hoisted.mockReadDaemonControlState.mockRejectedValueOnce(
      new DaemonRpcUnavailableError("daemon unavailable"),
    );

    const { ensureDesktopDaemonRuntime } =
      await import("../../packages/main/src/daemon-process.js");

    await expect(ensureDesktopDaemonRuntime()).resolves.toBeUndefined();
    expect(hoisted.mockStartConfiguredDaemon).toHaveBeenCalledTimes(1);
    expect(hoisted.mockReconcileDaemonControl).not.toHaveBeenCalled();
  });

  it("surfaces non-daemon-unavailable startup errors without starting", async () => {
    hoisted.mockReadDaemonControlState.mockRejectedValueOnce(new Error("daemon unavailable"));

    const { ensureDesktopDaemonRuntime } =
      await import("../../packages/main/src/daemon-process.js");

    await expect(ensureDesktopDaemonRuntime()).rejects.toThrow("daemon unavailable");
    expect(hoisted.mockStartConfiguredDaemon).not.toHaveBeenCalled();
    expect(hoisted.mockReconcileDaemonControl).not.toHaveBeenCalled();
  });

  it("re-reads live control state before local-mode quit shutdown", async () => {
    hoisted.mockReadDaemonControlState
      .mockResolvedValueOnce(
        createControlState({
          desiredMode: "background",
          allowedActions: ["stop"],
        }),
      )
      .mockResolvedValueOnce(
        createControlState({
          actualMode: "local",
          allowedActions: ["stop"],
          desiredMode: "local",
          message: "Local mode is the desired runtime.",
          daemonVersion: "0.0.1-test",
        }),
      );

    const { stopManagedDaemonOnQuit } = await import("../../packages/main/src/daemon-process.js");
    await stopManagedDaemonOnQuit();
    await stopManagedDaemonOnQuit();

    expect(hoisted.mockReadDaemonControlState).toHaveBeenCalledTimes(2);
    expect(hoisted.mockStopConfiguredDaemon).toHaveBeenCalledTimes(1);
  });

  it("does not stop on quit when Rust does not allow stop", async () => {
    hoisted.controlState = createControlState({
      actualMode: "stopped",
      allowedActions: ["start"],
      desiredMode: "local",
      message: "Local mode is the desired runtime.",
    });

    const { stopManagedDaemonOnQuit } = await import("../../packages/main/src/daemon-process.js");
    await stopManagedDaemonOnQuit();

    expect(hoisted.mockStopConfiguredDaemon).not.toHaveBeenCalled();
  });

  it("surfaces unavailable control state on quit without stopping", async () => {
    hoisted.mockReadDaemonControlState.mockRejectedValueOnce(new Error("daemon unavailable"));

    const { stopManagedDaemonOnQuit } = await import("../../packages/main/src/daemon-process.js");

    await expect(stopManagedDaemonOnQuit()).rejects.toThrow("daemon unavailable");
    expect(hoisted.mockStopConfiguredDaemon).not.toHaveBeenCalled();
  });
});
