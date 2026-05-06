import type { DaemonControlSnapshot } from "@taugentic/desktop-shared";
import type { PendingDaemonAction } from "./types";

export interface DaemonControlState {
  backgroundDisabled: boolean;
  backgroundLabel: string;
  backgroundNotice: string;
  disableBackgroundDisabled: boolean;
  disableBackgroundLabel: string;
  externalNotice: string | null;
  reconcileDisabled: boolean;
  reconcileLabel: string;
  startDisabled: boolean;
  startLabel: string;
  stopDisabled: boolean;
  stopLabel: string;
}

export function deriveDaemonControlState(
  state: DaemonControlSnapshot | null,
  pendingAction: PendingDaemonAction,
): DaemonControlState {
  const statusUnavailable = state === null;
  const desiredMode = state?.desiredMode ?? "local";
  const message =
    state?.message ??
    "Daemon status is unavailable. Start or reconcile the daemon to recover the desktop workspace.";
  const allowed = new Set(
    state?.allowedActions ?? (statusUnavailable ? ["start", "reconcile"] : []),
  );
  const reconcileRequired = state?.reconcileRequired ?? false;
  const transitionStatus = state?.transitionStatus ?? "idle";
  const externalRuntime = state?.actualMode === "foreign";

  return {
    backgroundDisabled: pendingAction !== null || !allowed.has("enableBackground"),
    backgroundLabel:
      pendingAction === "enable-background"
        ? "Enabling Background..."
        : desiredMode === "background"
          ? "Background Enabled"
          : "Enable Background",
    backgroundNotice: message,
    disableBackgroundDisabled: pendingAction !== null || !allowed.has("disableBackground"),
    disableBackgroundLabel:
      pendingAction === "disable-background" ? "Disabling Background..." : "Disable Background",
    externalNotice: externalRuntime ? message : null,
    reconcileDisabled: pendingAction !== null || !allowed.has("reconcile"),
    reconcileLabel:
      pendingAction === "reconcile"
        ? "Reconciling..."
        : statusUnavailable
          ? "Recover Daemon"
          : reconcileRequired || transitionStatus !== "idle"
            ? "Reconcile"
            : "Reconcile Runtime",
    startDisabled: pendingAction !== null || !allowed.has("start"),
    startLabel:
      pendingAction === "start"
        ? "Starting..."
        : desiredMode === "background"
          ? "Start Background"
          : "Start Local",
    stopDisabled: pendingAction !== null || !allowed.has("stop"),
    stopLabel: pendingAction === "stop" ? "Stopping..." : "Stop",
  };
}
