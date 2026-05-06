import type { DaemonControlSnapshot } from "@taugentic/desktop-shared";

export type PendingDaemonAction =
  | "refresh"
  | "start"
  | "stop"
  | "enable-background"
  | "disable-background"
  | "reconcile"
  | null;

export interface DaemonControlDeps {
  disableBackground: () => Promise<DaemonControlSnapshot>;
  enableBackground: () => Promise<DaemonControlSnapshot>;
  reconcile: () => Promise<DaemonControlSnapshot>;
  refresh: () => Promise<DaemonControlSnapshot>;
  start: () => Promise<DaemonControlSnapshot>;
  stop: () => Promise<DaemonControlSnapshot>;
}

export interface DaemonControlState {
  errorMessage: string | null;
  pendingAction: PendingDaemonAction;
  state: DaemonControlSnapshot | null;
}

export function createInitialDaemonControlState(): DaemonControlState {
  return {
    errorMessage: null,
    pendingAction: null,
    state: null,
  };
}
