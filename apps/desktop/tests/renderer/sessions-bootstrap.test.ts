import { describe, expect, it, vi } from "vite-plus/test";

import type { SessionId, SessionSummary } from "../../packages/shared/generated/index.js";
import { reconcilePersistedSessionSelection } from "../../packages/renderer/src/features/sessions/bootstrap.js";

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;

  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, reject, resolve };
}

describe("sessions bootstrap reconciliation", () => {
  it("clears the current session when no persisted selection exists and the session is gone", async () => {
    const onSessionChange = vi.fn<(sessionId: SessionId | null) => void>();

    await reconcilePersistedSessionSelection({
      deps: {
        listSessions: vi.fn(async () => [makeSession("session-2", "Two")]),
      },
      onSessionChange,
      readCurrentSessionId: () => "session-1",
      readPersistedSessionId: () => null,
    });

    expect(onSessionChange).toHaveBeenCalledWith(null);
  });

  it("keeps the current session when no persisted selection exists and it is still valid", async () => {
    const onSessionChange = vi.fn<(sessionId: SessionId | null) => void>();

    await reconcilePersistedSessionSelection({
      deps: {
        listSessions: vi.fn(async () => [makeSession("session-1", "One")]),
      },
      onSessionChange,
      readCurrentSessionId: () => "session-1",
      readPersistedSessionId: () => null,
    });

    expect(onSessionChange).not.toHaveBeenCalled();
  });

  it("clears a persisted session selection when the session no longer exists", async () => {
    const onSessionChange = vi.fn<(sessionId: SessionId | null) => void>();

    await reconcilePersistedSessionSelection({
      deps: {
        listSessions: vi.fn(async () => []),
      },
      onSessionChange,
      readCurrentSessionId: () => null,
      readPersistedSessionId: () => "session-1",
    });

    expect(onSessionChange).toHaveBeenCalledWith(null);
  });

  it("restores a persisted session selection when it is still locally visible", async () => {
    const onSessionChange = vi.fn<(sessionId: SessionId | null) => void>();

    await reconcilePersistedSessionSelection({
      deps: {
        listSessions: vi.fn<() => Promise<{ id: string; status: "idle"; title: string }[]>>(
          async () => [
            {
              id: "session-1",
              status: "idle",
              title: "One",
            },
          ],
        ),
      },
      onSessionChange,
      readCurrentSessionId: () => null,
      readPersistedSessionId: () => "session-1",
    });

    expect(onSessionChange).toHaveBeenCalledWith("session-1");
  });

  it("ignores a late missing-session result after the selection changes", async () => {
    const deferredSessions = createDeferred<SessionSummary[]>();
    const onSessionChange = vi.fn<(sessionId: SessionId | null) => void>();
    let currentSessionId: SessionId | null = null;

    const reconcilePromise = reconcilePersistedSessionSelection({
      deps: {
        listSessions: vi.fn<() => Promise<SessionSummary[]>>(() => deferredSessions.promise),
      },
      onSessionChange,
      readCurrentSessionId: () => currentSessionId,
      readPersistedSessionId: () => "session-1",
    });

    currentSessionId = "session-2";
    deferredSessions.resolve([]);
    await reconcilePromise;

    expect(onSessionChange).not.toHaveBeenCalled();
  });

  it("keeps the persisted selection when session lookup fails transiently", async () => {
    const onSessionChange = vi.fn<(sessionId: SessionId | null) => void>();

    await expect(
      reconcilePersistedSessionSelection({
        deps: {
          listSessions: vi.fn<() => Promise<SessionSummary[]>>(async () => {
            throw new Error("daemon unavailable");
          }),
        },
        onSessionChange,
        readCurrentSessionId: () => null,
        readPersistedSessionId: () => "session-1",
      }),
    ).resolves.toBeUndefined();
    expect(onSessionChange).not.toHaveBeenCalled();
  });
});

function makeSession(id: string, title: string): SessionSummary {
  return {
    id,
    status: "idle",
    title,
  };
}
