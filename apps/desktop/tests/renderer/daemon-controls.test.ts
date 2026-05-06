import { describe, expect, it } from "vite-plus/test";

import type { DaemonControlSnapshot } from "../../packages/shared/src/ipc.js";
import { deriveDaemonControlState } from "../../packages/renderer/src/features/daemon/controls.js";

function makeSnapshot(snapshot: Omit<DaemonControlSnapshot, "logPath">): DaemonControlSnapshot {
  return {
    ...snapshot,
    logPath: "/tmp/taugentic-daemon.log.jsonl",
  };
}

describe("deriveDaemonControlState", () => {
  it("surfaces external runtime notice from daemon status semantics", () => {
    const controls = deriveDaemonControlState(
      makeSnapshot({
        actualMode: "foreign",
        allowedActions: ["enableBackground"],
        backgroundOptIn: false,
        desiredMode: "local",
        errorCode: "externalRuntime",
        message: "Connected runtime is not owned by this control plane.",
        pendingTransition: null,
        protocolVersion: "2026-04-stage2",
        reconcileRequired: false,
        socketPath: "/tmp/ta-daemon.sock",
        transitionStatus: "idle",
      }),
      null,
    );

    expect(controls.startDisabled).toBe(true);
    expect(controls.stopDisabled).toBe(true);
    expect(controls.backgroundDisabled).toBe(false);
    expect(controls.externalNotice).toContain("not owned");
  });

  it("enables start and disable actions for a stopped desired background runtime", () => {
    const controls = deriveDaemonControlState(
      makeSnapshot({
        actualMode: "stopped",
        allowedActions: ["start", "disableBackground"],
        backgroundOptIn: true,
        desiredMode: "background",
        errorCode: null,
        message: "Background mode is the desired runtime.",
        pendingTransition: null,
        protocolVersion: "2026-04-stage2",
        reconcileRequired: false,
        socketPath: "/tmp/ta-daemon.sock",
        transitionStatus: "idle",
      }),
      null,
    );

    expect(controls.startDisabled).toBe(false);
    expect(controls.startLabel).toBe("Start Background");
    expect(controls.disableBackgroundDisabled).toBe(false);
    expect(controls.backgroundDisabled).toBe(true);
  });

  it("prefers reconcile action when the control plane is degraded", () => {
    const controls = deriveDaemonControlState(
      makeSnapshot({
        actualMode: "stopped",
        allowedActions: ["reconcile"],
        backgroundOptIn: true,
        desiredMode: "background",
        errorCode: "transitionFailed",
        message: "Enable background transition stalled.",
        pendingTransition: {
          kind: "enableBackground",
          opId: "7",
        },
        protocolVersion: "2026-04-stage2",
        reconcileRequired: true,
        socketPath: "/tmp/ta-daemon.sock",
        transitionStatus: "degradedReconcileRequired",
      }),
      null,
    );

    expect(controls.reconcileDisabled).toBe(false);
    expect(controls.reconcileLabel).toBe("Reconcile");
    expect(controls.startDisabled).toBe(true);
    expect(controls.backgroundNotice).toContain("stalled");
  });

  it("shows pending labels while the user action is in flight", () => {
    const controls = deriveDaemonControlState(
      makeSnapshot({
        actualMode: "local",
        allowedActions: ["stop", "enableBackground"],
        backgroundOptIn: false,
        desiredMode: "local",
        errorCode: null,
        message: "Local mode is the desired runtime.",
        pendingTransition: null,
        protocolVersion: "2026-04-stage2",
        reconcileRequired: false,
        socketPath: "/tmp/ta-daemon.sock",
        transitionStatus: "idle",
      }),
      "enable-background",
    );

    expect(controls.backgroundLabel).toBe("Enabling Background...");
    expect(controls.backgroundDisabled).toBe(true);
    expect(controls.stopDisabled).toBe(true);
    expect(controls.reconcileDisabled).toBe(true);
  });

  it("exposes explicit recovery actions when daemon status is unavailable", () => {
    const controls = deriveDaemonControlState(null, null);

    expect(controls.startDisabled).toBe(false);
    expect(controls.startLabel).toBe("Start Local");
    expect(controls.reconcileDisabled).toBe(false);
    expect(controls.reconcileLabel).toBe("Recover Daemon");
    expect(controls.backgroundDisabled).toBe(true);
    expect(controls.stopDisabled).toBe(true);
    expect(controls.backgroundNotice).toContain("Daemon status is unavailable");
    expect(controls.backgroundNotice).toContain("recover the desktop workspace");
  });
});
