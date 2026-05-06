import { beforeEach, describe, expect, it, vi } from "vite-plus/test";
import type { DaemonControlStatusResult } from "../../packages/shared/src/contracts.js";

function createControlState(
  overrides: Partial<DaemonControlStatusResult> = {},
): DaemonControlStatusResult {
  return {
    actualMode: "background",
    allowedActions: ["stop", "disableBackground"],
    backgroundOptIn: true,
    desiredMode: "background",
    errorCode: null,
    logPath: "/tmp/taugentic.log",
    message: "Background mode is the desired runtime.",
    daemonVersion: "0.0.1-test",
    pendingTransition: null,
    protocolVersion: "2026-04-stage2",
    reconcileRequired: false,
    socketPath: "/tmp/taugentic.sock",
    transitionStatus: "idle",
    ...overrides,
  };
}

type DaemonControlMock = () => Promise<DaemonControlStatusResult>;

const hoisted = vi.hoisted(() => ({
  DaemonRpcUnavailableError: class DaemonRpcUnavailableError extends Error {
    constructor(message = "daemon unavailable") {
      super(message);
      this.name = "DaemonRpcUnavailableError";
    }
  },
  controlState: createControlState(),
  mockStartDaemonViaBootstrap: vi.fn<DaemonControlMock>(async () => {
    throw new Error("unconfigured daemon start bootstrap mock");
  }),
  mockDisableDaemonBackgroundModeFromDaemon: vi.fn<DaemonControlMock>(async () => {
    throw new Error("unconfigured daemon disable mock");
  }),
  mockEnableDaemonBackgroundModeFromDaemon: vi.fn<DaemonControlMock>(async () => {
    throw new Error("unconfigured daemon enable mock");
  }),
  mockReconcileDaemonControlFromDaemon: vi.fn<DaemonControlMock>(async () => {
    throw new Error("unconfigured daemon reconcile mock");
  }),
  mockStopDaemonControlFromDaemon: vi.fn<DaemonControlMock>(async () => {
    throw new Error("unconfigured daemon stop mock");
  }),
  mockReadDaemonControlStateFromDaemon: vi.fn<DaemonControlMock>(async () => {
    throw new Error("unconfigured daemon state mock");
  }),
}));

vi.mock("../../packages/main/src/daemon-rpc-connection.js", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../../packages/main/src/daemon-rpc-connection.js")>();
  return {
    ...actual,
    DaemonRpcUnavailableError: hoisted.DaemonRpcUnavailableError,
  };
});

vi.mock("../../packages/main/src/daemon-rpc-client.js", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../../packages/main/src/daemon-rpc-client.js")>();
  return {
    ...actual,
    disableDaemonBackgroundModeFromDaemon: () =>
      hoisted.mockDisableDaemonBackgroundModeFromDaemon(),
    enableDaemonBackgroundModeFromDaemon: () => hoisted.mockEnableDaemonBackgroundModeFromDaemon(),
    reconcileDaemonControlFromDaemon: () => hoisted.mockReconcileDaemonControlFromDaemon(),
    readDaemonControlStateFromDaemon: () => hoisted.mockReadDaemonControlStateFromDaemon(),
    stopDaemonControlFromDaemon: () => hoisted.mockStopDaemonControlFromDaemon(),
  };
});

vi.mock("../../packages/main/src/daemon-bootstrap.js", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../../packages/main/src/daemon-bootstrap.js")>();
  return {
    ...actual,
    startDaemonViaBootstrap: () => hoisted.mockStartDaemonViaBootstrap(),
  };
});

describe("daemon-control", () => {
  beforeEach(() => {
    vi.resetModules();
    hoisted.mockStartDaemonViaBootstrap.mockReset();
    hoisted.mockDisableDaemonBackgroundModeFromDaemon.mockReset();
    hoisted.mockEnableDaemonBackgroundModeFromDaemon.mockReset();
    hoisted.mockReconcileDaemonControlFromDaemon.mockReset();
    hoisted.mockStopDaemonControlFromDaemon.mockReset();
    hoisted.mockReadDaemonControlStateFromDaemon.mockReset();

    hoisted.mockStartDaemonViaBootstrap.mockResolvedValue(hoisted.controlState);
    hoisted.mockDisableDaemonBackgroundModeFromDaemon.mockResolvedValue(hoisted.controlState);
    hoisted.mockEnableDaemonBackgroundModeFromDaemon.mockResolvedValue(hoisted.controlState);
    hoisted.mockReconcileDaemonControlFromDaemon.mockResolvedValue(hoisted.controlState);
    hoisted.mockStopDaemonControlFromDaemon.mockResolvedValue(hoisted.controlState);
    hoisted.mockReadDaemonControlStateFromDaemon.mockResolvedValue(hoisted.controlState);
  });

  it("prefers daemon socket status when reachable", async () => {
    hoisted.mockReadDaemonControlStateFromDaemon.mockResolvedValue(hoisted.controlState);
    const { readDaemonControlStateFromDaemon } =
      await import("../../packages/main/src/daemon-rpc-client.js");

    await expect(readDaemonControlStateFromDaemon()).resolves.toEqual(hoisted.controlState);
  });

  it("surfaces daemon unavailable when daemon status is unavailable", async () => {
    hoisted.mockReadDaemonControlStateFromDaemon.mockRejectedValueOnce(
      new hoisted.DaemonRpcUnavailableError(),
    );
    const { readDaemonControlStateFromDaemon } =
      await import("../../packages/main/src/daemon-rpc-client.js");

    await expect(readDaemonControlStateFromDaemon()).rejects.toThrow("daemon unavailable");
    expect(hoisted.mockReadDaemonControlStateFromDaemon).toHaveBeenCalledTimes(1);
  });

  it("routes mutating control through daemon rpc or ta-daemon bootstrap", async () => {
    const {
      disableDaemonBackgroundModeFromDaemon,
      enableDaemonBackgroundModeFromDaemon,
      reconcileDaemonControlFromDaemon,
      readDaemonControlStateFromDaemon,
      stopDaemonControlFromDaemon,
    } = await import("../../packages/main/src/daemon-rpc-client.js");
    const { startDaemonViaBootstrap } = await import("../../packages/main/src/daemon-bootstrap.js");

    await expect(readDaemonControlStateFromDaemon()).resolves.toEqual(hoisted.controlState);
    await expect(startDaemonViaBootstrap()).resolves.toEqual(hoisted.controlState);
    await expect(stopDaemonControlFromDaemon()).resolves.toEqual(hoisted.controlState);
    await expect(enableDaemonBackgroundModeFromDaemon()).resolves.toEqual(hoisted.controlState);
    await expect(disableDaemonBackgroundModeFromDaemon()).resolves.toEqual(hoisted.controlState);
    await expect(reconcileDaemonControlFromDaemon()).resolves.toEqual(hoisted.controlState);

    expect(hoisted.mockStartDaemonViaBootstrap).toHaveBeenCalledTimes(1);
    expect(hoisted.mockStopDaemonControlFromDaemon).toHaveBeenCalledTimes(1);
    expect(hoisted.mockEnableDaemonBackgroundModeFromDaemon).toHaveBeenCalledTimes(1);
    expect(hoisted.mockDisableDaemonBackgroundModeFromDaemon).toHaveBeenCalledTimes(1);
    expect(hoisted.mockReconcileDaemonControlFromDaemon).toHaveBeenCalledTimes(1);
    expect(hoisted.mockReadDaemonControlStateFromDaemon).toHaveBeenCalledTimes(1);
  });

  it("does not bootstrap start before enable when the daemon socket is unavailable", async () => {
    hoisted.mockEnableDaemonBackgroundModeFromDaemon.mockRejectedValueOnce(
      new hoisted.DaemonRpcUnavailableError(),
    );

    const { enableDaemonBackgroundModeFromDaemon } =
      await import("../../packages/main/src/daemon-rpc-client.js");

    await expect(enableDaemonBackgroundModeFromDaemon()).rejects.toThrow("daemon unavailable");
    expect(hoisted.mockStartDaemonViaBootstrap).not.toHaveBeenCalled();
    expect(hoisted.mockEnableDaemonBackgroundModeFromDaemon).toHaveBeenCalledTimes(1);
  });

  it("does not bootstrap start before reconcile when the daemon socket is unavailable", async () => {
    hoisted.mockReconcileDaemonControlFromDaemon.mockRejectedValueOnce(
      new hoisted.DaemonRpcUnavailableError(),
    );

    const { reconcileDaemonControlFromDaemon } =
      await import("../../packages/main/src/daemon-rpc-client.js");

    await expect(reconcileDaemonControlFromDaemon()).rejects.toThrow("daemon unavailable");
    expect(hoisted.mockStartDaemonViaBootstrap).not.toHaveBeenCalled();
    expect(hoisted.mockReconcileDaemonControlFromDaemon).toHaveBeenCalledTimes(1);
  });
});
