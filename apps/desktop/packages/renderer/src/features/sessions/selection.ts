import type { SessionId, SessionSummary } from "@taugentic/desktop-shared";
import { ProtocolValidationError, parseSessionId } from "@taugentic/desktop-shared/validation";

export const SELECTED_SESSION_STORAGE_KEY = "taugentic.desktop.selectedSessionId";

interface SessionSelectionStorage {
  getItem(key: string): string | null;
  removeItem(key: string): void;
  setItem(key: string, value: string): void;
}

export function isMissingSessionError(error: unknown, sessionId: SessionId): boolean {
  const detail = error instanceof Error ? error.message : String(error);
  return (
    detail.includes(`session does not exist: ${sessionId}`) ||
    detail.includes(`missing local session authority for ${sessionId}`) ||
    detail.includes(`session authority rejected: ${sessionId}`)
  );
}

export function shouldClearInvalidatedSession(
  currentSessionId: SessionId | null,
  invalidatedSessionId: SessionId,
): boolean {
  return currentSessionId === invalidatedSessionId;
}

export function reconcileCurrentSessionId(
  currentSessionId: SessionId | null,
  sessions: SessionSummary[],
): SessionId | null {
  if (currentSessionId === null) {
    return null;
  }

  return sessions.some((session) => session.id === currentSessionId) ? currentSessionId : null;
}

export function prependOpenedSession(
  sessions: SessionSummary[],
  openedSession: SessionSummary,
): SessionSummary[] {
  return [openedSession, ...sessions.filter((session) => session.id !== openedSession.id)];
}

export function loadPersistedCurrentSessionId(
  storage: SessionSelectionStorage | null = defaultSessionSelectionStorage(),
): SessionId | null {
  if (storage == null) {
    return null;
  }

  const storedSessionId = storage.getItem(SELECTED_SESSION_STORAGE_KEY)?.trim();
  if (!storedSessionId) {
    storage.removeItem(SELECTED_SESSION_STORAGE_KEY);
    return null;
  }

  try {
    return parseSessionId(storedSessionId);
  } catch (error) {
    if (error instanceof ProtocolValidationError) {
      storage.removeItem(SELECTED_SESSION_STORAGE_KEY);
      return null;
    }
    throw error;
  }
}

export function persistCurrentSessionId(
  sessionId: SessionId | null,
  storage: SessionSelectionStorage | null = defaultSessionSelectionStorage(),
): void {
  if (storage == null) {
    return;
  }

  if (sessionId === null) {
    storage.removeItem(SELECTED_SESSION_STORAGE_KEY);
    return;
  }

  storage.setItem(SELECTED_SESSION_STORAGE_KEY, sessionId);
}

function defaultSessionSelectionStorage(): SessionSelectionStorage | null {
  return typeof window === "undefined" ? null : window.localStorage;
}
