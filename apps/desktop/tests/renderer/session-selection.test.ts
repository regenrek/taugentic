import { describe, expect, it } from "vite-plus/test";

import type { SessionSummary } from "../../packages/shared/generated/SessionSummary.js";
import {
  isMissingSessionError,
  loadPersistedCurrentSessionId,
  persistCurrentSessionId,
  prependOpenedSession,
  reconcileCurrentSessionId,
  SELECTED_SESSION_STORAGE_KEY,
  shouldClearInvalidatedSession,
} from "../../packages/renderer/src/features/sessions/selection.js";

function makeSession(id: string, title: string): SessionSummary {
  return {
    id,
    status: "idle",
    title,
  };
}

describe("session selection", () => {
  function createMemoryStorage() {
    const values = new Map<string, string>();
    return {
      getItem(key: string) {
        return values.get(key) ?? null;
      },
      removeItem(key: string) {
        values.delete(key);
      },
      setItem(key: string, value: string) {
        values.set(key, value);
      },
    };
  }

  it("keeps the selected session only while it still exists in daemon-owned results", () => {
    expect(
      reconcileCurrentSessionId("session-2", [
        makeSession("session-1", "One"),
        makeSession("session-2", "Two"),
      ]),
    ).toBe("session-2");

    expect(reconcileCurrentSessionId("session-2", [makeSession("session-1", "One")])).toBeNull();
  });

  it("prepends a newly opened daemon-owned session without duplicating it", () => {
    const opened = makeSession("session-2", "Two");

    expect(prependOpenedSession([makeSession("session-1", "One"), opened], opened)).toEqual([
      opened,
      makeSession("session-1", "One"),
    ]);
  });

  it("loads and persists the selected session id through local storage", () => {
    const storage = createMemoryStorage();

    persistCurrentSessionId("session-2", storage);
    expect(storage.getItem(SELECTED_SESSION_STORAGE_KEY)).toBe("session-2");
    expect(loadPersistedCurrentSessionId(storage)).toBe("session-2");

    persistCurrentSessionId(null, storage);
    expect(storage.getItem(SELECTED_SESSION_STORAGE_KEY)).toBeNull();
    expect(loadPersistedCurrentSessionId(storage)).toBeNull();
  });

  it("purges invalid persisted selected session ids", () => {
    const storage = createMemoryStorage();

    storage.setItem(SELECTED_SESSION_STORAGE_KEY, "   ");
    expect(loadPersistedCurrentSessionId(storage)).toBeNull();
    expect(storage.getItem(SELECTED_SESSION_STORAGE_KEY)).toBeNull();
  });

  it("clears the persisted selected session id once daemon-owned session results drop it", () => {
    const storage = createMemoryStorage();

    persistCurrentSessionId("session-2", storage);
    const reconciled = reconcileCurrentSessionId("session-2", [makeSession("session-1", "One")]);
    persistCurrentSessionId(reconciled, storage);

    expect(reconciled).toBeNull();
    expect(storage.getItem(SELECTED_SESSION_STORAGE_KEY)).toBeNull();
  });

  it("detects missing-session errors from the desktop IPC boundary", () => {
    expect(
      isMissingSessionError(
        new Error(
          "Error invoking remote method 'desktop:list-runs': DaemonJsonRpcError: daemon JSON-RPC error -32602: session does not exist: session-2",
        ),
        "session-2",
      ),
    ).toBe(true);
    expect(
      isMissingSessionError(
        new Error(
          "Error invoking remote method 'desktop:list-runs': DaemonJsonRpcError: daemon JSON-RPC error -32602: session authority rejected: session-2",
        ),
        "session-2",
      ),
    ).toBe(true);
    expect(
      isMissingSessionError(
        new Error(
          "Error invoking remote method 'desktop:list-runs': DaemonProtocolError: missing local session authority for session-2",
        ),
        "session-2",
      ),
    ).toBe(true);
    expect(isMissingSessionError(new Error("daemon unavailable"), "session-2")).toBe(false);
  });

  it("clears only the currently selected invalidated session", () => {
    expect(shouldClearInvalidatedSession("session-2", "session-2")).toBe(true);
    expect(shouldClearInvalidatedSession("session-3", "session-2")).toBe(false);
    expect(shouldClearInvalidatedSession(null, "session-2")).toBe(false);
  });
});
