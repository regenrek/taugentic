import { describe, expect, it, vi } from "vite-plus/test";

import type { RunStreamEventEnvelope } from "../../packages/shared/src/ipc.js";
import type { ActivityPageResult, RunSummary } from "../../packages/shared/src/contracts.js";
import {
  createSessionRunsActor,
  type SessionRunsMachineDeps,
} from "../../packages/renderer/src/features/runs/model.js";

function makeRun(id: string, status: RunSummary["status"]): RunSummary {
  return {
    id,
    objective: `objective-${id}`,
    runtimeProfileId: "runtime-profile-1",
    status,
  };
}

function makeRunEvent(sequence: bigint): RunStreamEventEnvelope {
  return {
    daemonInstanceId: "daemon-1",
    sessionId: "session-1",
    occurredAtMs: sequence * 10n,
    sequence,
    event: {
      run: {
        detail: `detail-${sequence.toString()}`,
        runId: `run-${sequence.toString()}`,
        status: "running",
      },
    },
  };
}

function makeActivityPage(
  ...events: Array<{ runId: string; status: RunSummary["status"]; sequence: bigint }>
): ActivityPageResult {
  return {
    items: events.map((event) => ({
      cursor: {
        sequence: event.sequence,
      },
      occurredAtMs: event.sequence * 10n,
      event: {
        run: {
          detail: `detail-${event.runId}-${event.status}`,
          runId: event.runId,
          status: event.status,
        },
      },
    })),
    latestActivityCursor:
      events.length === 0
        ? null
        : {
            sequence: events[0].sequence,
          },
    nextBefore: null,
  };
}

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;

  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, reject, resolve };
}

function createFakeSubscription() {
  let onError: ((error: Error) => void) | undefined;
  let onMessage:
    | ((
        message:
          | RunStreamEventEnvelope
          | { stream: "runs"; status: "historyGap" | "ready" | "terminalError" },
      ) => void)
    | undefined;
  const unsubscribe = vi.fn();

  return {
    emit(
      message:
        | RunStreamEventEnvelope
        | { stream: "runs"; status: "historyGap" | "ready" | "terminalError" },
    ) {
      onMessage?.(message);
    },
    fail(message: string) {
      onError?.(new Error(message));
    },
    subscribeRunStream: vi.fn(
      async (
        _sessionId: string,
        _afterCursor: ActivityPageResult["latestActivityCursor"],
        nextOnMessage: (
          message:
            | RunStreamEventEnvelope
            | { stream: "runs"; status: "historyGap" | "ready" | "terminalError" },
        ) => void,
        nextOnError?: (error: Error) => void,
      ) => {
        onMessage = nextOnMessage;
        onError = nextOnError;
        return unsubscribe;
      },
    ),
    unsubscribe,
  };
}

function createDeps(overrides: Partial<SessionRunsMachineDeps> = {}): SessionRunsMachineDeps {
  return {
    hydrateSnapshot: vi.fn(),
    loadSnapshot: vi.fn(async () => ({
      activityPage: makeActivityPage(),
      runs: [],
    })),
    subscribeRunStream: createFakeSubscription().subscribeRunStream,
    startRun: vi.fn(async () => {}),
    ...overrides,
  };
}

async function flushMicrotasks(turns = 20): Promise<void> {
  for (let index = 0; index < turns; index += 1) {
    await Promise.resolve();
  }
}

describe("run stream machine", () => {
  it("hydrates query-owned runs and activity before opening the live stream", async () => {
    const stream = createFakeSubscription();
    const snapshot = {
      activityPage: makeActivityPage({
        runId: "run-1",
        status: "waitingForApproval",
        sequence: 3n,
      }),
      runs: [makeRun("run-1", "waitingForApproval")],
    };
    const loadSnapshot = vi.fn(async () => snapshot);
    const hydrateSnapshot = vi.fn();
    const actor = createSessionRunsActor({
      deps: createDeps({
        hydrateSnapshot,
        loadSnapshot,
        subscribeRunStream: stream.subscribeRunStream,
      }),
      sessionId: "session-1",
    });

    actor.start();
    await flushMicrotasks();

    expect(loadSnapshot).toHaveBeenCalledWith("session-1");
    expect(hydrateSnapshot).toHaveBeenCalledWith(snapshot);
    expect(stream.subscribeRunStream).toHaveBeenCalledWith(
      "session-1",
      { sequence: 3n },
      expect.any(Function),
      expect.any(Function),
    );
    expect(actor.getSnapshot().context).toMatchObject({
      errorMessage: null,
      isHydrating: false,
      streamStatus: "live",
    });
  });

  it("does not open the live stream when hydration fails", async () => {
    const stream = createFakeSubscription();
    const onMissingSession = vi.fn<(sessionId: string) => void>();
    const actor = createSessionRunsActor({
      deps: createDeps({
        loadSnapshot: vi.fn(async () => {
          throw new Error("run read failed");
        }),
        subscribeRunStream: stream.subscribeRunStream,
      }),
      onSessionInvalid: onMissingSession,
      sessionId: "session-9",
    });

    actor.start();
    await flushMicrotasks();

    expect(stream.subscribeRunStream).not.toHaveBeenCalled();
    expect(onMissingSession).not.toHaveBeenCalled();
    expect(actor.getSnapshot().context).toMatchObject({
      errorMessage: "run stream failed for session-9: run read failed",
      isHydrating: false,
      streamStatus: "error",
    });
  });

  it("signals the session owner when hydration reports a missing session", async () => {
    const onMissingSession = vi.fn<(sessionId: string) => void>();
    const actor = createSessionRunsActor({
      deps: createDeps({
        loadSnapshot: vi.fn(async () => {
          throw new Error(
            "Error invoking remote method 'desktop:list-runs': DaemonJsonRpcError: daemon JSON-RPC error -32602: session does not exist: session-871b622eb4bb4de5a9f9bc147a51e6bb",
          );
        }),
      }),
      onSessionInvalid: onMissingSession,
      sessionId: "session-871b622eb4bb4de5a9f9bc147a51e6bb",
    });

    actor.start();
    await flushMicrotasks();

    expect(onMissingSession).toHaveBeenCalledWith("session-871b622eb4bb4de5a9f9bc147a51e6bb");
  });

  it("opens the live stream with a null cursor when no activity cursor is hydrated", async () => {
    const stream = createFakeSubscription();
    const actor = createSessionRunsActor({
      deps: createDeps({
        loadSnapshot: vi.fn(async () => ({
          activityPage: makeActivityPage(),
          runs: [],
        })),
        subscribeRunStream: stream.subscribeRunStream,
      }),
      sessionId: "session-11",
    });

    actor.start();
    await flushMicrotasks();

    expect(stream.subscribeRunStream).toHaveBeenCalledWith(
      "session-11",
      null,
      expect.any(Function),
      expect.any(Function),
    );
  });

  it("cleans up a late stream subscription when the actor stops during subscribe", async () => {
    const subscribeRunStream = createDeferred<() => void>();
    const unsubscribe = vi.fn();
    const actor = createSessionRunsActor({
      deps: createDeps({
        subscribeRunStream: vi.fn(() => subscribeRunStream.promise),
      }),
      sessionId: "session-7",
    });

    actor.start();
    await flushMicrotasks();
    actor.stop();
    subscribeRunStream.resolve(unsubscribe);
    await flushMicrotasks();

    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("tolerates no-op stream unsubscribers during teardown", async () => {
    const actor = createSessionRunsActor({
      deps: createDeps({
        subscribeRunStream: vi.fn(async () => () => undefined),
      }),
      sessionId: "session-8",
    });

    actor.start();
    await flushMicrotasks();

    expect(() => actor.stop()).not.toThrow();
  });

  it("refreshes query-owned run data whenever a live run event arrives", async () => {
    const stream = createFakeSubscription();
    const loadSnapshot = vi
      .fn<() => Promise<{ activityPage: ActivityPageResult; runs: RunSummary[] }>>()
      .mockResolvedValueOnce({
        activityPage: makeActivityPage(),
        runs: [makeRun("run-1", "waitingForApproval")],
      })
      .mockResolvedValueOnce({
        activityPage: makeActivityPage({ runId: "run-1", status: "completed", sequence: 9n }),
        runs: [makeRun("run-1", "completed")],
      });
    const hydrateSnapshot = vi.fn();
    const actor = createSessionRunsActor({
      deps: createDeps({
        hydrateSnapshot,
        loadSnapshot: vi.fn(() => loadSnapshot()),
        subscribeRunStream: stream.subscribeRunStream,
      }),
      sessionId: "session-4",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit(makeRunEvent(9n));
    await flushMicrotasks();

    expect(loadSnapshot).toHaveBeenCalledTimes(2);
    expect(hydrateSnapshot).toHaveBeenNthCalledWith(2, {
      activityPage: makeActivityPage({ runId: "run-1", status: "completed", sequence: 9n }),
      runs: [makeRun("run-1", "completed")],
    });
    expect(actor.getSnapshot().context).toMatchObject({
      errorMessage: null,
      isHydrating: false,
      streamStatus: "live",
    });
  });

  it("queues a trailing refresh when a second live event lands during an in-flight refresh", async () => {
    const stream = createFakeSubscription();
    const deferredSnapshot = createDeferred<{
      activityPage: ActivityPageResult;
      runs: RunSummary[];
    }>();
    const loadSnapshot = vi
      .fn<() => Promise<{ activityPage: ActivityPageResult; runs: RunSummary[] }>>()
      .mockResolvedValueOnce({
        activityPage: makeActivityPage(),
        runs: [makeRun("run-1", "waitingForApproval")],
      })
      .mockImplementationOnce(() => deferredSnapshot.promise)
      .mockResolvedValueOnce({
        activityPage: makeActivityPage({ runId: "run-2", status: "running", sequence: 10n }),
        runs: [makeRun("run-2", "running")],
      });
    const hydrateSnapshot = vi.fn();
    const actor = createSessionRunsActor({
      deps: createDeps({
        hydrateSnapshot,
        loadSnapshot: vi.fn(() => loadSnapshot()),
        subscribeRunStream: stream.subscribeRunStream,
      }),
      sessionId: "session-13",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit(makeRunEvent(9n));
    await flushMicrotasks();
    stream.emit(makeRunEvent(10n));
    await flushMicrotasks();
    deferredSnapshot.resolve({
      activityPage: makeActivityPage({ runId: "run-1", status: "completed", sequence: 9n }),
      runs: [makeRun("run-1", "completed")],
    });
    await flushMicrotasks();

    expect(loadSnapshot).toHaveBeenCalledTimes(3);
    expect(hydrateSnapshot).toHaveBeenLastCalledWith({
      activityPage: makeActivityPage({ runId: "run-2", status: "running", sequence: 10n }),
      runs: [makeRun("run-2", "running")],
    });
  });

  it("rehydrates query-owned run data when main requests reconnect recovery", async () => {
    const stream = createFakeSubscription();
    const loadSnapshot = vi
      .fn<() => Promise<{ activityPage: ActivityPageResult; runs: RunSummary[] }>>()
      .mockResolvedValueOnce({
        activityPage: makeActivityPage({
          runId: "run-1",
          status: "waitingForApproval",
          sequence: 3n,
        }),
        runs: [makeRun("run-1", "waitingForApproval")],
      })
      .mockResolvedValueOnce({
        activityPage: makeActivityPage({ runId: "run-1", status: "running", sequence: 12n }),
        runs: [makeRun("run-1", "running")],
      });
    const hydrateSnapshot = vi.fn();
    const actor = createSessionRunsActor({
      deps: createDeps({
        hydrateSnapshot,
        loadSnapshot: vi.fn(() => loadSnapshot()),
        subscribeRunStream: stream.subscribeRunStream,
      }),
      sessionId: "session-10",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit({ stream: "runs", status: "historyGap" });
    await flushMicrotasks();

    expect(loadSnapshot).toHaveBeenCalledTimes(2);
    expect(hydrateSnapshot).toHaveBeenLastCalledWith({
      activityPage: makeActivityPage({ runId: "run-1", status: "running", sequence: 12n }),
      runs: [makeRun("run-1", "running")],
    });
  });

  it("marks the run stream errored on terminal status envelopes", async () => {
    const stream = createFakeSubscription();
    const actor = createSessionRunsActor({
      deps: createDeps({
        loadSnapshot: vi.fn(async () => ({
          activityPage: makeActivityPage({
            runId: "run-1",
            status: "waitingForApproval",
            sequence: 3n,
          }),
          runs: [makeRun("run-1", "waitingForApproval")],
        })),
        subscribeRunStream: stream.subscribeRunStream,
      }),
      sessionId: "session-12",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit({ stream: "runs", status: "terminalError" });
    await flushMicrotasks();

    expect(actor.getSnapshot().context).toMatchObject({
      errorMessage: "run stream entered a terminal error state",
      isHydrating: false,
      streamStatus: "error",
    });
  });

  it("marks the run stream errored on decode failures", async () => {
    const stream = createFakeSubscription();
    const actor = createSessionRunsActor({
      deps: createDeps({
        subscribeRunStream: stream.subscribeRunStream,
      }),
      sessionId: "session-6",
    });

    actor.start();
    await flushMicrotasks();
    stream.fail("run stream decode failed for session-6");
    await flushMicrotasks();

    expect(actor.getSnapshot().context.errorMessage).toBe("run stream decode failed for session-6");
    expect(actor.getSnapshot().context.streamStatus).toBe("error");
  });

  it("automatically retries after a stream failure", async () => {
    vi.useFakeTimers();
    try {
      const subscribeRunStream = vi
        .fn<SessionRunsMachineDeps["subscribeRunStream"]>()
        .mockRejectedValueOnce(new Error("socket down"))
        .mockResolvedValueOnce(() => undefined);
      const loadSnapshot = vi
        .fn<() => Promise<{ activityPage: ActivityPageResult; runs: RunSummary[] }>>()
        .mockResolvedValue({
          activityPage: makeActivityPage(),
          runs: [],
        });
      const actor = createSessionRunsActor({
        deps: createDeps({
          hydrateSnapshot: vi.fn(),
          loadSnapshot: vi.fn(() => loadSnapshot()),
          subscribeRunStream: vi.fn((sessionId, afterCursor, onMessage, onError) =>
            subscribeRunStream(sessionId, afterCursor, onMessage, onError),
          ),
        }),
        sessionId: "session-retry",
      });

      actor.start();
      await flushMicrotasks();

      expect(actor.getSnapshot().context.streamStatus).toBe("error");

      await vi.advanceTimersByTimeAsync(1_500);
      await flushMicrotasks();

      expect(loadSnapshot).toHaveBeenCalledTimes(2);
      expect(subscribeRunStream).toHaveBeenCalledTimes(2);
      expect(actor.getSnapshot().context.streamStatus).toBe("live");

      actor.stop();
    } finally {
      vi.useRealTimers();
    }
  });
});
