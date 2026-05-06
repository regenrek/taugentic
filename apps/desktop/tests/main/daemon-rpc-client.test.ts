import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { type DaemonControlStatusResult } from "../../packages/shared/src/index.js";

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
  controlState: createControlState(),
  mockSnapshotDaemonViaBootstrap: vi.fn<DaemonControlMock>(async () => createControlState()),
  mockEnableDaemonBackgroundModeViaBootstrap: vi.fn<DaemonControlMock>(async () =>
    createControlState(),
  ),
  mockDisableDaemonBackgroundModeViaBootstrap: vi.fn<DaemonControlMock>(async () =>
    createControlState(),
  ),
  mockReconcileDaemonViaBootstrap: vi.fn<DaemonControlMock>(async () => createControlState()),
  mockStopDaemonViaBootstrap: vi.fn<DaemonControlMock>(async () => createControlState()),
}));

vi.mock("../../packages/main/src/daemon-bootstrap.js", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("../../packages/main/src/daemon-bootstrap.js")>();
  return {
    ...actual,
    disableDaemonBackgroundModeViaBootstrap: () =>
      hoisted.mockDisableDaemonBackgroundModeViaBootstrap(),
    enableDaemonBackgroundModeViaBootstrap: () =>
      hoisted.mockEnableDaemonBackgroundModeViaBootstrap(),
    reconcileDaemonViaBootstrap: () => hoisted.mockReconcileDaemonViaBootstrap(),
    snapshotDaemonViaBootstrap: () => hoisted.mockSnapshotDaemonViaBootstrap(),
    stopDaemonViaBootstrap: () => hoisted.mockStopDaemonViaBootstrap(),
  };
});

describe("daemon-rpc-client", () => {
  beforeEach(() => {
    vi.resetModules();
    hoisted.mockSnapshotDaemonViaBootstrap.mockReset();
    hoisted.mockSnapshotDaemonViaBootstrap.mockResolvedValue(hoisted.controlState);
    hoisted.mockEnableDaemonBackgroundModeViaBootstrap.mockReset();
    hoisted.mockDisableDaemonBackgroundModeViaBootstrap.mockReset();
    hoisted.mockReconcileDaemonViaBootstrap.mockReset();
    hoisted.mockStopDaemonViaBootstrap.mockReset();
    hoisted.mockEnableDaemonBackgroundModeViaBootstrap.mockResolvedValue(hoisted.controlState);
    hoisted.mockDisableDaemonBackgroundModeViaBootstrap.mockResolvedValue(hoisted.controlState);
    hoisted.mockReconcileDaemonViaBootstrap.mockResolvedValue(hoisted.controlState);
    hoisted.mockStopDaemonViaBootstrap.mockResolvedValue(hoisted.controlState);
  });

  it("routes daemon control reads and mutations through local bootstrap", async () => {
    const {
      disableDaemonBackgroundModeFromDaemon,
      enableDaemonBackgroundModeFromDaemon,
      readDaemonControlStateFromDaemon,
      reconcileDaemonControlFromDaemon,
      stopDaemonControlFromDaemon,
    } = await import("../../packages/main/src/daemon-rpc-client.js");

    await expect(readDaemonControlStateFromDaemon()).resolves.toEqual(hoisted.controlState);
    await expect(enableDaemonBackgroundModeFromDaemon()).resolves.toEqual(hoisted.controlState);
    await expect(disableDaemonBackgroundModeFromDaemon()).resolves.toEqual(hoisted.controlState);
    await expect(reconcileDaemonControlFromDaemon()).resolves.toEqual(hoisted.controlState);
    await expect(stopDaemonControlFromDaemon()).resolves.toEqual(hoisted.controlState);

    expect(hoisted.mockSnapshotDaemonViaBootstrap).toHaveBeenCalledTimes(1);
    expect(hoisted.mockEnableDaemonBackgroundModeViaBootstrap).toHaveBeenCalledTimes(1);
    expect(hoisted.mockDisableDaemonBackgroundModeViaBootstrap).toHaveBeenCalledTimes(1);
    expect(hoisted.mockReconcileDaemonViaBootstrap).toHaveBeenCalledTimes(1);
    expect(hoisted.mockStopDaemonViaBootstrap).toHaveBeenCalledTimes(1);
  });

  it("propagates bootstrap snapshot failures unchanged", async () => {
    const unavailable = new Error("daemon unavailable");
    hoisted.mockSnapshotDaemonViaBootstrap.mockRejectedValueOnce(unavailable);

    const { readDaemonControlStateFromDaemon } =
      await import("../../packages/main/src/daemon-rpc-client.js");

    await expect(readDaemonControlStateFromDaemon()).rejects.toBe(unavailable);
  });
});
