import { afterEach, describe, expect, it } from "vite-plus/test";

import type {
  SessionId,
  SessionSummary,
  WorkspaceSelector,
} from "../../packages/shared/generated/index.js";
import {
  createInitialSessionsPanelState,
  selectSessionsPanelSnapshotContext,
  unwrapSessionsRefetchResult,
} from "../../packages/renderer/src/features/sessions/model.js";
import {
  createSessionsPanelStore,
  disposeSessionsPanelStore,
  getSessionsPanelState,
  openSessionsPanelSession,
  refreshSessionsPanel,
  setSessionsPanelDraftTitle,
} from "../../packages/renderer/src/features/sessions/store.js";
import {
  getCurrentWorkspaceSessionId,
  resetWorkspaceShellForTests,
  workspaceShellStore,
} from "../../packages/renderer/src/features/workspace/state/store.js";

type SessionsRefetchResult = Parameters<typeof unwrapSessionsRefetchResult>[0];

function makeSession(id: string, title: string): SessionSummary {
  return {
    id,
    status: "idle",
    title,
  };
}

const testWorkspaceSelector: WorkspaceSelector = {
  id: "workspace-test-default",
  kind: "byId",
};

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;

  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, reject, resolve };
}

function createStoreDriver(initialSessionId: SessionId | null = "session-1") {
  const store = createSessionsPanelStore();
  const sessionChanges: Array<SessionId | null> = [];
  workspaceShellStore.trigger.sessionChanged({ sessionId: initialSessionId });

  function handleSessionChange(nextSessionId: SessionId | null) {
    workspaceShellStore.trigger.sessionChanged({ sessionId: nextSessionId });
    sessionChanges.push(nextSessionId);
  }

  return {
    get currentSessionId() {
      return getCurrentWorkspaceSessionId();
    },
    get sessionChanges() {
      return sessionChanges;
    },
    get state() {
      return getSessionsPanelState(store);
    },
    handleSessionChange,
    store,
  };
}

afterEach(() => {
  resetWorkspaceShellForTests();
});

describe("sessions panel store", () => {
  it("starts from the default panel state", () => {
    const store = createSessionsPanelStore();

    expect(getSessionsPanelState(store)).toEqual(createInitialSessionsPanelState());
    expect(Object.keys(getSessionsPanelState(store)).sort()).toEqual([
      "draftTitle",
      "errorMessage",
      "pendingAction",
    ]);
  });

  it("returns the stable store context for React selectors", () => {
    const store = createSessionsPanelStore();
    const snapshot = store.getSnapshot();

    expect(selectSessionsPanelSnapshotContext(snapshot)).toBe(snapshot.context);
  });

  it("ignores refresh requests while an open session request is in flight", async () => {
    const open = createDeferred<SessionSummary>();
    const refreshSessions = async () => [makeSession("session-1", "One")];
    const driver = createStoreDriver("session-1");

    setSessionsPanelDraftTitle(driver.store, "New session");
    const openPromise = openSessionsPanelSession(
      driver.store,
      {
        openSession: () => open.promise,
      },
      testWorkspaceSelector,
      driver.handleSessionChange,
    );
    await refreshSessionsPanel(driver.store, refreshSessions, driver.handleSessionChange);

    open.resolve(makeSession("session-3", "Three"));
    await openPromise;

    expect(driver.currentSessionId).toBe("session-3");
    expect(driver.sessionChanges).toEqual(["session-3"]);
  });

  it("does not let a stale refresh overwrite a newer opened session", async () => {
    const refresh = createDeferred<SessionSummary[]>();
    const open = createDeferred<SessionSummary>();
    const driver = createStoreDriver("session-1");
    const deps = {
      openSession: () => open.promise,
    };

    const refreshPromise = refreshSessionsPanel(
      driver.store,
      () => refresh.promise,
      driver.handleSessionChange,
    );
    setSessionsPanelDraftTitle(driver.store, "New session");
    const openPromise = openSessionsPanelSession(
      driver.store,
      deps,
      testWorkspaceSelector,
      driver.handleSessionChange,
    );

    open.resolve(makeSession("session-3", "Three"));
    await openPromise;

    expect(driver.currentSessionId).toBe("session-3");
    expect(driver.state.pendingAction).toBeNull();
    expect(driver.state.draftTitle).toBe("");

    refresh.resolve([makeSession("session-1", "One")]);
    await refreshPromise;

    expect(driver.currentSessionId).toBe("session-3");
    expect(driver.sessionChanges).toEqual(["session-3"]);
  });

  it("reconciles a refresh against the latest selected session from the workspace shell", async () => {
    const refresh = createDeferred<SessionSummary[]>();
    const driver = createStoreDriver("session-1");

    const refreshPromise = refreshSessionsPanel(
      driver.store,
      () => refresh.promise,
      driver.handleSessionChange,
    );
    driver.handleSessionChange("session-2");

    refresh.resolve([makeSession("session-1", "One"), makeSession("session-2", "Two")]);
    await refreshPromise;

    expect(driver.currentSessionId).toBe("session-2");
    expect(driver.sessionChanges).toEqual(["session-2"]);
  });

  it("preserves the current selection when refresh fails", async () => {
    const driver = createStoreDriver("session-1");

    await refreshSessionsPanel(
      driver.store,
      async () => {
        throw new Error("daemon unavailable");
      },
      driver.handleSessionChange,
    );

    expect(driver.currentSessionId).toBe("session-1");
    expect(driver.sessionChanges).toEqual([]);
    expect(driver.state).toEqual({
      draftTitle: "New coding session",
      errorMessage: "daemon unavailable",
      pendingAction: null,
    });
  });

  it("drops late async writes after the store is deactivated", async () => {
    const refresh = createDeferred<SessionSummary[]>();
    const driver = createStoreDriver("session-1");

    const refreshPromise = refreshSessionsPanel(
      driver.store,
      () => refresh.promise,
      driver.handleSessionChange,
    );
    disposeSessionsPanelStore(driver.store);

    refresh.resolve([makeSession("session-1", "One")]);
    await refreshPromise;

    expect(driver.sessionChanges).toEqual([]);
    expect(driver.state).toEqual({
      draftTitle: "New coding session",
      errorMessage: null,
      pendingAction: "refresh",
    });
  });

  it("throws when a query refetch resolves with an error instead of session data", () => {
    expect(() =>
      unwrapSessionsRefetchResult({
        data: undefined,
        error: new Error("daemon unavailable"),
      } as SessionsRefetchResult),
    ).toThrow("daemon unavailable");
  });
});
