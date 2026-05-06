import { describe, expect, it, vi } from "vite-plus/test";

import type { ActivityPageResult } from "../../packages/shared/src/contracts.js";
import {
  createSessionRunsActor,
  type SessionRunsMachineDeps,
} from "../../packages/renderer/src/features/runs/model.js";

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;

  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, reject, resolve };
}

function createActivityPage(): ActivityPageResult {
  return {
    items: [],
    latestActivityCursor: null,
    nextBefore: null,
  };
}

function createDeps(overrides: Partial<SessionRunsMachineDeps> = {}): SessionRunsMachineDeps {
  return {
    hydrateSnapshot: vi.fn(),
    loadSnapshot: vi.fn(async () => ({
      activityPage: createActivityPage(),
      runs: [],
    })),
    subscribeRunStream: vi.fn(async () => () => undefined),
    startRun: vi.fn(async () => {}),
    ...overrides,
  };
}

async function flushMicrotasks(turns = 20): Promise<void> {
  for (let index = 0; index < turns; index += 1) {
    await Promise.resolve();
  }
}

describe("runs domain command machine", () => {
  it("requires a non-empty run objective", async () => {
    const deps = createDeps();
    const actor = createSessionRunsActor({
      deps,
      sessionId: "session-1",
    });

    actor.start();
    await flushMicrotasks();
    actor.send({
      type: "draftChanged",
      value: "   ",
    });
    actor.send({
      type: "startRequested",
    });
    await flushMicrotasks();

    expect(deps.startRun).not.toHaveBeenCalled();
    expect(actor.getSnapshot().context.commandErrorMessage).toBe("Run objective is required.");
    expect(actor.getSnapshot().context.draftObjective).toBe("   ");
    expect(
      actor.getSnapshot().matches({
        command: "idle",
      }),
    ).toBe(true);
  });

  it("does not notify a completed run after the actor stops", async () => {
    const deferredStart = createDeferred<void>();
    const onRunStarted = vi.fn();
    const actor = createSessionRunsActor({
      deps: createDeps({
        startRun: vi.fn(() => deferredStart.promise),
      }),
      onRunStarted,
      sessionId: "session-1",
    });

    actor.start();
    await flushMicrotasks();
    actor.send({
      type: "startRequested",
    });
    actor.stop();
    deferredStart.resolve();
    await flushMicrotasks();

    expect(onRunStarted).not.toHaveBeenCalled();
  });

  it("starts a run with the trimmed objective and clears the draft on success", async () => {
    const startRun = vi.fn(async () => {});
    const onRunStarted = vi.fn();
    const actor = createSessionRunsActor({
      deps: createDeps({
        startRun,
      }),
      onRunStarted,
      sessionId: "session-2",
    });

    actor.start();
    await flushMicrotasks();
    actor.send({
      type: "draftChanged",
      value: "  Ship app server hard cut  ",
    });
    actor.send({
      type: "startRequested",
    });
    await flushMicrotasks();

    expect(startRun).toHaveBeenCalledWith("session-2", "Ship app server hard cut");
    expect(onRunStarted).toHaveBeenCalledTimes(1);
    expect(actor.getSnapshot().context.commandErrorMessage).toBeNull();
    expect(actor.getSnapshot().context.draftObjective).toBe("");
  });
});
