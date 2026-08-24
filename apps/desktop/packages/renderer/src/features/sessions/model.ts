import { useState } from "react";

import { useQueryClient } from "@tanstack/react-query";

import { useSelector } from "@xstate/store/react";

import type { SessionId, SessionSummary, Workspace } from "@taugentic/desktop-shared";

import { openSession } from "../../lib/ipc/api";
import { sessionOverviewRootKey, queryKeys } from "../../lib/queries/keys";
import { useSessionsQuery, type SessionQueryView } from "../../lib/queries/session-queries";
import { useMountEffect } from "../../lib/react/use-mount-effect";
import { useWorkspacePicker } from "../workspace/useWorkspacePicker";
import {
  createInitialSessionsPanelState,
  createSessionsPanelStore,
  disposeSessionsPanelStore,
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
  trustWorkspacePath: string | null;
}

export type SessionOpenResult = "opened" | "cancelled" | "trustRequired" | "failed";

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
  const [pendingTrust, setPendingTrust] = useState<{
    sequence: number;
    title: string;
  } | null>(null);
  const qc = useQueryClient();
  const sessionsQuery = useSessionsQuery();
  const workspacePicker = useWorkspacePicker();
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
    trustWorkspacePath: workspacePicker.trustPath,
  };

  useMountEffect(() => {
    return () => disposeSessionsPanelStore(store);
  });

  return {
    state,
    cancelWorkspaceTrust: () => {
      workspacePicker.cancelTrust();
      if (pendingTrust) {
        store.trigger.openFailed({
          errorMessage: "Workspace trust is required before opening a session.",
          sequence: pendingTrust.sequence,
        });
      }
      setPendingTrust(null);
    },
    confirmWorkspaceTrust: async (): Promise<boolean> => {
      if (pendingTrust === null) {
        return false;
      }
      try {
        const result = await workspacePicker.confirmTrust();
        if (result.status !== "opened") {
          throw new Error("Workspace trust confirmation did not open a workspace.");
        }
        await finishOpenSession(result.workspace, pendingTrust.title, pendingTrust.sequence);
        return true;
      } catch (error) {
        store.trigger.openFailed({
          errorMessage: error instanceof Error ? error.message : String(error),
          sequence: pendingTrust.sequence,
        });
        return false;
      } finally {
        setPendingTrust(null);
      }
    },
    openSession: async (): Promise<SessionOpenResult> => {
      const snapshot = store.getSnapshot().context;
      if (snapshot.pendingAction === "open") {
        return "failed";
      }
      const title = snapshot.draftTitle.trim();
      if (!title) {
        store.trigger.openFailed({
          errorMessage: "Session title is required.",
          sequence: snapshot.requestSequence,
        });
        return "failed";
      }

      const sequence = snapshot.requestSequence + 1;
      store.trigger.openStarted({ sequence });
      try {
        const result = await workspacePicker.pickWorkspace();
        if (result.status === "cancelled") {
          store.trigger.openFailed({
            errorMessage: "Workspace selection cancelled.",
            sequence,
          });
          return "cancelled";
        }
        if (result.status === "trustRequired") {
          setPendingTrust({ sequence, title });
          return "trustRequired";
        }
        await finishOpenSession(result.workspace, title, sequence);
        return "opened";
      } catch (error) {
        store.trigger.openFailed({
          errorMessage: error instanceof Error ? error.message : String(error),
          sequence,
        });
        return "failed";
      }
    },
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

  async function finishOpenSession(
    workspace: Workspace,
    title: string,
    sequence: number,
  ): Promise<void> {
    const openedSession = await deps.openSession(title, {
      kind: "byId",
      id: workspace.id,
    });
    store.trigger.openSucceeded({ sequence });
    qc.setQueryData<SessionSummary[]>(queryKeys.sessions, (currentSessions) =>
      prependOpenedSession(currentSessions ?? [], openedSession),
    );
    void qc.invalidateQueries({ queryKey: sessionOverviewRootKey });
    onSessionChange(openedSession.id);
  }
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
