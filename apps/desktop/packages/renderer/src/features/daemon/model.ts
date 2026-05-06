import { useActorRef, useSelector } from "@xstate/react";

import type { DaemonControlSnapshot } from "@taugentic/desktop-shared";

import {
  disableBackgroundService,
  enableBackgroundService,
  getDaemonStatus,
  reconcileDaemon,
  startDaemon,
  stopDaemon,
} from "../../lib/ipc/api";
import { useMountEffect } from "../../lib/react/use-mount-effect";
import { daemonControlMachine, requestDaemonControlAction } from "./state/machine";
import type { DaemonControlDeps, PendingDaemonAction } from "./types";

export interface DaemonControlModel {
  readonly errorMessage: string | null;
  readonly pendingAction: PendingDaemonAction;
  readonly state: DaemonControlSnapshot | null;
  disableBackground: () => Promise<void>;
  enableBackground: () => Promise<void>;
  reconcile: () => Promise<void>;
  refresh: () => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
}

export interface DaemonShellSummary {
  readonly isDegraded: boolean;
  readonly statusLabel: string;
  readonly unavailableSummary: string;
  readonly unavailableTitle: string;
}

const defaultDeps: DaemonControlDeps = {
  disableBackground: disableBackgroundService,
  enableBackground: enableBackgroundService,
  reconcile: reconcileDaemon,
  refresh: getDaemonStatus,
  start: startDaemon,
  stop: stopDaemon,
};

export function useDaemonControlModel(deps: DaemonControlDeps = defaultDeps): DaemonControlModel {
  const actorRef = useActorRef(daemonControlMachine, {
    input: {
      deps,
    },
  });
  const state = useSelector(actorRef, (snapshot) => snapshot.context);

  useMountEffect(() => {
    void requestDaemonControlAction(actorRef, "refresh");
  });

  return {
    disableBackground: () => requestDaemonControlAction(actorRef, "disable-background"),
    enableBackground: () => requestDaemonControlAction(actorRef, "enable-background"),
    errorMessage: state.errorMessage,
    pendingAction: state.pendingAction,
    reconcile: () => requestDaemonControlAction(actorRef, "reconcile"),
    refresh: () => requestDaemonControlAction(actorRef, "refresh"),
    state: state.state,
    start: () => requestDaemonControlAction(actorRef, "start"),
    stop: () => requestDaemonControlAction(actorRef, "stop"),
  };
}

export function deriveDaemonShellSummary({
  errorMessage,
  pendingAction,
  state,
}: Pick<DaemonControlModel, "errorMessage" | "pendingAction" | "state">): DaemonShellSummary {
  const unavailableSummary =
    "Session work stays paused until daemon status returns in Daemon Status.";
  const unavailableTitle = "Daemon unavailable";

  if (state === null) {
    return {
      isDegraded: errorMessage !== null,
      statusLabel: errorMessage === null ? "loading daemon status" : "daemon unavailable",
      unavailableSummary,
      unavailableTitle,
    };
  }

  if (pendingAction !== null) {
    return {
      isDegraded: false,
      statusLabel: `${state.actualMode} · ${pendingAction}`,
      unavailableSummary,
      unavailableTitle,
    };
  }

  return {
    isDegraded: false,
    statusLabel: `${state.actualMode} · ${state.transitionStatus}`,
    unavailableSummary,
    unavailableTitle,
  };
}
