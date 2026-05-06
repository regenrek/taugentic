import { createStore } from "@xstate/store";

import type { SessionId, SessionSummary } from "@taugentic/desktop-shared";

import { getCurrentWorkspaceSessionId } from "../workspace/state/store";
import { reconcileCurrentSessionId } from "./selection";

export type SessionsPanelPendingAction = "open" | "refresh" | null;

export type SessionsPanelState = {
  draftTitle: string;
  errorMessage: string | null;
  pendingAction: SessionsPanelPendingAction;
};

interface SessionsPanelContext extends SessionsPanelState {
  active: boolean;
  requestSequence: number;
}

export interface SessionsPanelDeps {
  openSession: (title: string) => Promise<SessionSummary>;
}

export type SessionsPanelStore = ReturnType<typeof createSessionsPanelStore>;

const DEFAULT_DRAFT_TITLE = "New coding session";

export function createInitialSessionsPanelState(): SessionsPanelState {
  return {
    draftTitle: DEFAULT_DRAFT_TITLE,
    errorMessage: null,
    pendingAction: null,
  };
}

function createInitialSessionsPanelContext(): SessionsPanelContext {
  return {
    ...createInitialSessionsPanelState(),
    active: true,
    requestSequence: 0,
  };
}

export function createSessionsPanelStore() {
  return createStore({
    context: createInitialSessionsPanelContext(),
    on: {
      deactivated: (context) => ({
        ...context,
        active: false,
        requestSequence: context.requestSequence + 1,
      }),
      draftTitleChanged: (
        context,
        event: {
          value: string;
        },
      ) => ({
        ...context,
        draftTitle: event.value,
      }),
      openFailed: (
        context,
        event: {
          errorMessage: string;
          sequence: number;
        },
      ) => {
        if (!isActiveRequest(context, event.sequence)) {
          return context;
        }
        return {
          ...context,
          errorMessage: event.errorMessage,
          pendingAction: null,
        };
      },
      openStarted: (
        context,
        event: {
          sequence: number;
        },
      ) => ({
        ...context,
        errorMessage: null,
        pendingAction: "open" as const,
        requestSequence: event.sequence,
      }),
      openSucceeded: (
        context,
        event: {
          sequence: number;
        },
      ) => {
        if (!isActiveRequest(context, event.sequence)) {
          return context;
        }
        return {
          ...context,
          draftTitle: "",
          pendingAction: null,
        };
      },
      refreshFailed: (
        context,
        event: {
          errorMessage: string;
          sequence: number;
        },
      ) => {
        if (!isActiveRequest(context, event.sequence)) {
          return context;
        }
        return {
          ...context,
          errorMessage: event.errorMessage,
          pendingAction: null,
        };
      },
      refreshStarted: (
        context,
        event: {
          sequence: number;
        },
      ) => ({
        ...context,
        errorMessage: null,
        pendingAction: "refresh" as const,
        requestSequence: event.sequence,
      }),
      refreshSucceeded: (
        context,
        event: {
          sequence: number;
        },
      ) => {
        if (!isActiveRequest(context, event.sequence)) {
          return context;
        }
        return {
          ...context,
          pendingAction: null,
        };
      },
    },
  });
}

export function disposeSessionsPanelStore(store: SessionsPanelStore): void {
  store.trigger.deactivated();
}

export function getSessionsPanelState(store: SessionsPanelStore): SessionsPanelState {
  const { draftTitle, errorMessage, pendingAction } = store.getSnapshot().context;
  return {
    draftTitle,
    errorMessage,
    pendingAction,
  };
}

export function setSessionsPanelDraftTitle(store: SessionsPanelStore, value: string): void {
  store.trigger.draftTitleChanged({ value });
}

export async function openSessionsPanelSession(
  store: SessionsPanelStore,
  deps: SessionsPanelDeps,
  onSessionChange: (sessionId: SessionId | null) => void,
  onOpenedSession?: (openedSession: SessionSummary) => void,
): Promise<void> {
  const snapshot = store.getSnapshot().context;
  if (snapshot.pendingAction === "open") {
    return;
  }

  const title = snapshot.draftTitle.trim();
  if (!title) {
    store.trigger.openFailed({
      errorMessage: "Session title is required.",
      sequence: snapshot.requestSequence,
    });
    return;
  }

  const sequence = snapshot.requestSequence + 1;
  store.trigger.openStarted({ sequence });

  try {
    const openedSession = await deps.openSession(title);
    store.trigger.openSucceeded({ sequence });
    if (!matchesCurrentRequest(store, sequence)) {
      return;
    }
    onOpenedSession?.(openedSession);
    onSessionChange(openedSession.id);
  } catch (error) {
    store.trigger.openFailed({
      errorMessage: error instanceof Error ? error.message : String(error),
      sequence,
    });
  }
}

export async function refreshSessionsPanel(
  store: SessionsPanelStore,
  refreshSessions: () => Promise<SessionSummary[]>,
  onSessionChange: (sessionId: SessionId | null) => void,
): Promise<void> {
  const snapshot = store.getSnapshot().context;
  if (snapshot.pendingAction !== null) {
    return;
  }

  const sequence = snapshot.requestSequence + 1;
  store.trigger.refreshStarted({ sequence });

  try {
    const sessions = await refreshSessions();
    store.trigger.refreshSucceeded({ sequence });
    if (!matchesCurrentRequest(store, sequence)) {
      return;
    }

    const currentSessionId = getCurrentWorkspaceSessionId();
    const nextCurrentSessionId = reconcileCurrentSessionId(currentSessionId, sessions);
    if (nextCurrentSessionId !== currentSessionId) {
      onSessionChange(nextCurrentSessionId);
    }
  } catch (error) {
    store.trigger.refreshFailed({
      errorMessage: error instanceof Error ? error.message : String(error),
      sequence,
    });
  }
}

function isActiveRequest(context: SessionsPanelContext, sequence: number): boolean {
  return context.active && context.requestSequence === sequence;
}

function matchesCurrentRequest(store: SessionsPanelStore, sequence: number): boolean {
  return isActiveRequest(store.getSnapshot().context, sequence);
}
