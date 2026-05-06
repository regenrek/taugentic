import { useState } from "react";

import { useQueryClient } from "@tanstack/react-query";

import { useSelector } from "@xstate/store/react";

import type { SessionId, SessionSummary } from "@taugentic/desktop-shared";

import { openSession } from "../../lib/ipc/api";
import { sessionOverviewRootKey, queryKeys } from "../../lib/queries/keys";
import { useSessionsQuery, type SessionQueryView } from "../../lib/queries/session-queries";
import { useMountEffect } from "../../lib/react/use-mount-effect";
import {
  createInitialSessionsPanelState,
  createSessionsPanelStore,
  disposeSessionsPanelStore,
  openSessionsPanelSession,
  refreshSessionsPanel,
  setSessionsPanelDraftTitle,
  type SessionsPanelDeps,
  type SessionsPanelStore,
} from "./store";
import { prependOpenedSession } from "./selection";

export { createInitialSessionsPanelState };
export type { SessionsPanelDeps };

type SessionsPanelSnapshotContext = ReturnType<SessionsPanelStore["getSnapshot"]>["context"];
type SessionsRefetchResult = Awaited<ReturnType<SessionQueryView<SessionSummary[]>["refetch"]>>;

export interface SessionsPanelViewState {
  draftTitle: string;
  errorMessage: string | null;
  pendingAction: "open" | "refresh" | null;
  sessions: SessionSummary[];
}

export function selectSessionsPanelSnapshotContext(
  snapshot: ReturnType<SessionsPanelStore["getSnapshot"]>,
): SessionsPanelSnapshotContext {
  return snapshot.context;
}

export function unwrapSessionsRefetchResult(result: SessionsRefetchResult): SessionSummary[] {
  if (result.error) {
    throw result.error;
  }
  if (result.data) {
    return result.data;
  }
  throw new Error("session refresh returned no data");
}

export function useSessionsPanelModel(
  _currentSessionId: SessionId | null,
  onSessionChange: (sessionId: SessionId | null) => void,
  deps: SessionsPanelDeps = {
    openSession,
  },
) {
  const [store] = useState(() => createSessionsPanelStore());
  const qc = useQueryClient();
  const sessionsQuery = useSessionsQuery();
  const snapshotContext = useSelector(store, selectSessionsPanelSnapshotContext);
  const state: SessionsPanelViewState = {
    draftTitle: snapshotContext.draftTitle,
    errorMessage:
      snapshotContext.errorMessage ??
      (sessionsQuery.error instanceof Error
        ? sessionsQuery.error.message
        : sessionsQuery.error
          ? toUnknownErrorMessage(sessionsQuery.error)
          : null),
    pendingAction: snapshotContext.pendingAction,
    sessions: sessionsQuery.data ?? [],
  };

  useMountEffect(() => {
    return () => disposeSessionsPanelStore(store);
  });

  return {
    state,
    openSession: () =>
      openSessionsPanelSession(store, deps, onSessionChange, (openedSession) => {
        qc.setQueryData<SessionSummary[]>(queryKeys.sessions, (currentSessions) =>
          prependOpenedSession(currentSessions ?? [], openedSession),
        );
        void qc.invalidateQueries({ queryKey: sessionOverviewRootKey });
      }),
    refreshSessions: () =>
      refreshSessionsPanel(
        store,
        async () => {
          const result = await sessionsQuery.refetch();
          return unwrapSessionsRefetchResult(result);
        },
        onSessionChange,
      ),
    selectSession: (sessionId: SessionId) => onSessionChange(sessionId),
    setDraftTitle: (value: string) => setSessionsPanelDraftTitle(store, value),
  };
}

function toUnknownErrorMessage(error: unknown): string {
  if (
    typeof error === "string" ||
    typeof error === "number" ||
    typeof error === "boolean" ||
    typeof error === "bigint"
  ) {
    return String(error);
  }
  return JSON.stringify(error) ?? "unknown error";
}
