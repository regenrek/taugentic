import { describe, expect, it } from "vite-plus/test";
import type { createActor as createXStateActor } from "../../packages/renderer/node_modules/xstate/dist/declarations/src/index.js";

import type { DaemonControlSnapshot } from "../../packages/shared/src/ipc.js";
import {
  daemonControlMachine,
  requestDaemonControlAction,
} from "../../packages/renderer/src/features/daemon/state/machine.js";

const xstateModulePath = "../../packages/renderer/node_modules/xstate/dist/xstate.esm.js";

const { createActor } = (await import(xstateModulePath)) as {
  createActor: typeof createXStateActor;
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

function makeSnapshot(
  actualMode: DaemonControlSnapshot["actualMode"],
  transitionStatus: DaemonControlSnapshot["transitionStatus"] = "idle",
): DaemonControlSnapshot {
  return {
    actualMode,
    allowedActions: ["start", "stop", "reconcile", "enableBackground", "disableBackground"],
    backgroundOptIn: actualMode === "background",
    desiredMode: actualMode === "background" ? "background" : "local",
    errorCode: null,
    message: `${actualMode} snapshot`,
    pendingTransition: null,
    protocolVersion: "2026-04-stage2",
    reconcileRequired: false,
    logPath: "/tmp/taugentic-daemon.log.jsonl",
    socketPath: "/tmp/ta-daemon.sock",
    transitionStatus,
  };
}

describe("daemon control model", () => {
  it("does not let a stale refresh overwrite a newer start result", async () => {
    const refresh = createDeferred<DaemonControlSnapshot>();
    const start = createDeferred<DaemonControlSnapshot>();
    const actor = createActor(daemonControlMachine, {
      input: {
        deps: {
          disableBackground: async () => makeSnapshot("local"),
          enableBackground: async () => makeSnapshot("background"),
          reconcile: async () => makeSnapshot("local"),
          refresh: () => refresh.promise,
          start: () => start.promise,
          stop: async () => makeSnapshot("stopped"),
        },
      },
    });
    type ActorSnapshot = ReturnType<typeof actor.getSnapshot>;
    const snapshots: ActorSnapshot["context"][] = [];
    actor.subscribe((snapshot: ActorSnapshot) => {
      snapshots.push(snapshot.context);
    });
    actor.start();

    const refreshPromise = requestDaemonControlAction(actor, "refresh");
    const startPromise = requestDaemonControlAction(actor, "start");

    start.resolve(makeSnapshot("local"));
    await startPromise;

    expect(actor.getSnapshot().context.state?.actualMode).toBe("local");
    expect(actor.getSnapshot().context.pendingAction).toBeNull();

    refresh.resolve(makeSnapshot("stopped"));
    await refreshPromise;

    expect(actor.getSnapshot().context.state?.actualMode).toBe("local");
    expect(actor.getSnapshot().context.pendingAction).toBeNull();
    expect(snapshots.at(-1)?.state?.actualMode).toBe("local");
  });

  it("does not let a stale refresh error clobber a newer reconcile result", async () => {
    const refresh = createDeferred<DaemonControlSnapshot>();
    const reconcile = createDeferred<DaemonControlSnapshot>();
    const actor = createActor(daemonControlMachine, {
      input: {
        deps: {
          disableBackground: async () => makeSnapshot("local"),
          enableBackground: async () => makeSnapshot("background"),
          reconcile: () => reconcile.promise,
          refresh: () => refresh.promise,
          start: async () => makeSnapshot("local"),
          stop: async () => makeSnapshot("stopped"),
        },
      },
    });
    actor.start();

    const refreshPromise = requestDaemonControlAction(actor, "refresh");
    const reconcilePromise = requestDaemonControlAction(actor, "reconcile");

    reconcile.resolve(makeSnapshot("local", "degradedReconcileRequired"));
    await reconcilePromise;

    refresh.reject(new Error("stale refresh failed"));
    await refreshPromise;

    expect(actor.getSnapshot().context.errorMessage).toBeNull();
    expect(actor.getSnapshot().context.pendingAction).toBeNull();
    expect(actor.getSnapshot().context.state?.transitionStatus).toBe("degradedReconcileRequired");
  });

  it("drops late async writes after stop", async () => {
    const refresh = createDeferred<DaemonControlSnapshot>();
    const actor = createActor(daemonControlMachine, {
      input: {
        deps: {
          disableBackground: async () => makeSnapshot("local"),
          enableBackground: async () => makeSnapshot("background"),
          reconcile: async () => makeSnapshot("local"),
          refresh: () => refresh.promise,
          start: async () => makeSnapshot("local"),
          stop: async () => makeSnapshot("stopped"),
        },
      },
    });
    type ActorSnapshot = ReturnType<typeof actor.getSnapshot>;
    const snapshots: ActorSnapshot["context"][] = [];
    actor.subscribe((snapshot: ActorSnapshot) => {
      snapshots.push(snapshot.context);
    });
    actor.start();

    void requestDaemonControlAction(actor, "refresh");
    actor.stop();

    refresh.resolve(makeSnapshot("local"));
    await refresh.promise;

    expect(snapshots.at(-1)).toEqual({
      completion: expect.any(Function),
      deps: expect.any(Object),
      errorMessage: null,
      pendingAction: "refresh",
      state: null,
    });
    expect(actor.getSnapshot().context).toEqual({
      completion: expect.any(Function),
      deps: expect.any(Object),
      errorMessage: null,
      pendingAction: "refresh",
      state: null,
    });
  });
});
