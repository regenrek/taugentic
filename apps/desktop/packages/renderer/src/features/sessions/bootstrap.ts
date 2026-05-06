import type { SessionId, SessionSummary } from "@taugentic/desktop-shared";

import { listSessions } from "../../lib/ipc/api";
import { useMountEffect } from "../../lib/react/use-mount-effect";
import { loadPersistedCurrentSessionId } from "./selection";
import { getCurrentWorkspaceSessionId } from "../workspace/state/store";

export interface SessionBootstrapDeps {
  listSessions(): Promise<SessionSummary[]>;
}

type ReconcilePersistedSessionSelectionOptions = {
  deps: SessionBootstrapDeps;
  onSessionChange: (sessionId: SessionId | null) => void;
  readCurrentSessionId?: () => SessionId | null;
  readPersistedSessionId?: () => SessionId | null;
};

const defaultDeps: SessionBootstrapDeps = {
  listSessions,
};

export async function reconcilePersistedSessionSelection({
  deps,
  onSessionChange,
  readCurrentSessionId = getCurrentWorkspaceSessionId,
  readPersistedSessionId = loadPersistedCurrentSessionId,
}: ReconcilePersistedSessionSelectionOptions): Promise<void> {
  const currentSessionId = readCurrentSessionId();
  const persistedSessionId = readPersistedSessionId();
  const requestedSessionId = currentSessionId ?? persistedSessionId;
  if (requestedSessionId === null) {
    return;
  }

  let sessions: SessionSummary[];
  try {
    sessions = await deps.listSessions();
  } catch {
    return;
  }

  const latestCurrentSessionId = readCurrentSessionId();
  if (latestCurrentSessionId !== null && latestCurrentSessionId !== requestedSessionId) {
    return;
  }

  const nextSessionId = sessions.some((session) => session.id === requestedSessionId)
    ? requestedSessionId
    : null;
  if (nextSessionId === null) {
    onSessionChange(null);
    return;
  }
  if (latestCurrentSessionId !== nextSessionId) {
    onSessionChange(nextSessionId);
  }
}

export function usePersistedSessionBootstrap(
  onSessionChange: (sessionId: SessionId | null) => void,
  deps: SessionBootstrapDeps = defaultDeps,
): void {
  useMountEffect(() => {
    let disposed = false;

    void reconcilePersistedSessionSelection({
      deps,
      onSessionChange: (sessionId) => {
        if (!disposed) {
          onSessionChange(sessionId);
        }
      },
      readCurrentSessionId: () => (disposed ? null : getCurrentWorkspaceSessionId()),
    });

    return () => {
      disposed = true;
    };
  });
}
