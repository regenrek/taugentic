import { describe, expect, it, vi } from "vite-plus/test";

import type { ApprovalSnapshotResult } from "../../packages/shared/generated/ApprovalSnapshotResult.js";
import type { ApprovalRequest } from "../../packages/shared/generated/ApprovalRequest.js";
import type {
  ApprovalStreamEventEnvelope,
  ApprovalStreamMessage,
} from "../../packages/shared/src/ipc.js";
import {
  createSessionApprovalActor,
  type SessionApprovalActorDeps,
} from "../../packages/renderer/src/features/approvals/connection.js";

function makeApprovalRequest(id: string, reason: string): ApprovalRequest {
  return {
    expiresAtMs: 60_000n,
    id,
    reason,
    requestedAtMs: 0n,
    runId: "run-1",
    scope: "processExec",
    target: { kind: "processExec", command: "echo ok" },
  };
}

function makeApprovalEvent(sequence: bigint): ApprovalStreamEventEnvelope {
  return {
    daemonInstanceId: "daemon-1",
    sessionId: "session-4",
    sequence,
    occurredAtMs: sequence * 10n,
    event: {
      approval: {
        phase: "requested",
        request: makeApprovalRequest(`approval-${sequence.toString()}`, "need shell"),
      },
    },
  };
}

function makeApprovalSnapshot(
  items: ApprovalRequest[],
  sequence: bigint | null = null,
): ApprovalSnapshotResult {
  return {
    items,
    latestCursor:
      sequence == null
        ? null
        : {
            daemonInstanceId: "daemon-1",
            sessionId: "session-4",
            sequence,
          },
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
  let onMessage: ((message: ApprovalStreamMessage) => void) | undefined;
  const unsubscribe = vi.fn();

  return {
    emit(message: ApprovalStreamMessage) {
      onMessage?.(message);
    },
    fail(message: string) {
      onError?.(new Error(message));
    },
    subscribeApprovalStream: vi.fn(
      async (
        _sessionId: string,
        _afterCursor: ApprovalSnapshotResult["latestCursor"],
        nextOnMessage: (message: ApprovalStreamMessage) => void,
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

async function flushMicrotasks(turns = 16): Promise<void> {
  for (let index = 0; index < turns; index += 1) {
    await Promise.resolve();
  }
}

describe("approval connection", () => {
  it("hydrates query-owned approvals before opening the live stream", async () => {
    const stream = createFakeSubscription();
    const snapshot = makeApprovalSnapshot([makeApprovalRequest("approval-1", "need shell")], 12n);
    const listApprovals = vi.fn(async () => snapshot);
    const hydrateSnapshot = vi.fn();
    const openApprovalStream = vi.fn(stream.subscribeApprovalStream);
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {}),
        hydrateSnapshot,
        listApprovals,
        subscribeApprovalStream: openApprovalStream,
      },
      sessionId: "session-1",
    });

    actor.start();
    await flushMicrotasks();

    expect(listApprovals).toHaveBeenCalledWith("session-1", {});
    expect(hydrateSnapshot).toHaveBeenCalledWith(snapshot);
    expect(actor.getSnapshot().context).toMatchObject({
      commandErrorMessage: null,
      errorMessage: null,
      lastSequence: 12n,
      pendingApprovalId: null,
      pendingDecision: null,
      sessionId: "session-1",
      streamStatus: "ready",
    });
    expect(openApprovalStream).toHaveBeenCalledWith(
      "session-1",
      {
        daemonInstanceId: "daemon-1",
        sessionId: "session-4",
        sequence: 12n,
      },
      expect.any(Function),
      expect.any(Function),
    );
  });

  it("does not open the live stream when hydration fails", async () => {
    const stream = createFakeSubscription();
    const openApprovalStream = vi.fn(stream.subscribeApprovalStream);
    const onMissingSession = vi.fn<(sessionId: string) => void>();
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {}),
        hydrateSnapshot: vi.fn(),
        listApprovals: vi.fn(async () => {
          throw new Error("approval read failed");
        }),
        subscribeApprovalStream: openApprovalStream,
      },
      onMissingSession,
      sessionId: "session-9",
    });

    actor.start();
    await flushMicrotasks();

    expect(openApprovalStream).not.toHaveBeenCalled();
    expect(onMissingSession).not.toHaveBeenCalled();
    expect(actor.getSnapshot().context).toMatchObject({
      errorMessage: "approval stream failed for session-9: approval read failed",
      lastSequence: null,
      sessionId: "session-9",
      streamStatus: "error",
    });
  });

  it("signals the session owner when approval hydration hits a missing session", async () => {
    const onMissingSession = vi.fn<(sessionId: string) => void>();
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {}),
        hydrateSnapshot: vi.fn(),
        listApprovals: vi.fn(async () => {
          throw new Error(
            "Error invoking remote method 'desktop:list-approvals': DaemonJsonRpcError: daemon JSON-RPC error -32602: session does not exist: session-2",
          );
        }),
        subscribeApprovalStream: vi.fn(createFakeSubscription().subscribeApprovalStream),
      },
      onMissingSession,
      sessionId: "session-2",
    });

    actor.start();
    await flushMicrotasks();

    expect(onMissingSession).toHaveBeenCalledWith("session-2");
  });

  it("cleans up a late stream subscription when the view unmounts during subscribe", async () => {
    const openApprovalStream = createDeferred<() => void>();
    const unsubscribe = vi.fn();
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {}),
        hydrateSnapshot: vi.fn(),
        listApprovals: vi.fn(async () => makeApprovalSnapshot([], null)),
        subscribeApprovalStream: vi.fn(() => openApprovalStream.promise),
      },
      sessionId: "session-7",
    });

    actor.start();
    await flushMicrotasks();

    actor.stop();
    openApprovalStream.resolve(unsubscribe);
    await flushMicrotasks();

    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("passes null as the first cursor when hydration has no daemon cursor yet", async () => {
    const stream = createFakeSubscription();
    const openApprovalStream = vi.fn(stream.subscribeApprovalStream);
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {}),
        hydrateSnapshot: vi.fn(),
        listApprovals: vi.fn(async () => makeApprovalSnapshot([], null)),
        subscribeApprovalStream: openApprovalStream,
      },
      sessionId: "session-11",
    });

    actor.start();
    await flushMicrotasks();

    expect(openApprovalStream).toHaveBeenCalledWith(
      "session-11",
      null,
      expect.any(Function),
      expect.any(Function),
    );
  });

  it("tolerates no-op stream unsubscribers during teardown", async () => {
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {}),
        hydrateSnapshot: vi.fn(),
        listApprovals: vi.fn(async () => makeApprovalSnapshot([], null)),
        subscribeApprovalStream: vi.fn(async () => () => undefined),
      },
      sessionId: "session-8",
    });

    actor.start();
    await flushMicrotasks();

    expect(() => actor.stop()).not.toThrow();
  });

  it("refreshes query-owned approvals whenever a live approval event arrives", async () => {
    const stream = createFakeSubscription();
    const listApprovals = vi
      .fn<() => Promise<ApprovalSnapshotResult>>()
      .mockResolvedValueOnce(
        makeApprovalSnapshot([makeApprovalRequest("approval-1", "old reason")], 10n),
      )
      .mockResolvedValueOnce(
        makeApprovalSnapshot([makeApprovalRequest("approval-1", "new reason")], 41n),
      );
    const hydrateSnapshot = vi.fn();
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {}),
        hydrateSnapshot,
        listApprovals: vi.fn(() => listApprovals()),
        subscribeApprovalStream: vi.fn(stream.subscribeApprovalStream),
      },
      sessionId: "session-4",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit(makeApprovalEvent(41n));
    await flushMicrotasks();

    expect(listApprovals).toHaveBeenCalledTimes(2);
    expect(hydrateSnapshot).toHaveBeenNthCalledWith(
      2,
      makeApprovalSnapshot([makeApprovalRequest("approval-1", "new reason")], 41n),
    );
    expect(actor.getSnapshot().context).toMatchObject({
      errorMessage: null,
      lastSequence: 41n,
      sessionId: "session-4",
      streamStatus: "ready",
    });
  });

  it("queues a trailing approval refresh when a second live event lands during an in-flight refresh", async () => {
    const stream = createFakeSubscription();
    const deferredSnapshot = createDeferred<ApprovalSnapshotResult>();
    const listApprovals = vi
      .fn<() => Promise<ApprovalSnapshotResult>>()
      .mockResolvedValueOnce(
        makeApprovalSnapshot([makeApprovalRequest("approval-1", "old reason")], 10n),
      )
      .mockImplementationOnce(() => deferredSnapshot.promise)
      .mockResolvedValueOnce(
        makeApprovalSnapshot([makeApprovalRequest("approval-2", "new reason")], 12n),
      );
    const hydrateSnapshot = vi.fn();
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {}),
        hydrateSnapshot,
        listApprovals: vi.fn(() => listApprovals()),
        subscribeApprovalStream: vi.fn(stream.subscribeApprovalStream),
      },
      sessionId: "session-13",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit(makeApprovalEvent(11n));
    await flushMicrotasks();
    stream.emit(makeApprovalEvent(12n));
    await flushMicrotasks();
    deferredSnapshot.resolve(
      makeApprovalSnapshot([makeApprovalRequest("approval-1", "intermediate")], 11n),
    );
    await flushMicrotasks();

    expect(listApprovals).toHaveBeenCalledTimes(3);
    expect(hydrateSnapshot).toHaveBeenLastCalledWith(
      makeApprovalSnapshot([makeApprovalRequest("approval-2", "new reason")], 12n),
    );
    expect(actor.getSnapshot().context).toMatchObject({
      errorMessage: null,
      lastSequence: 12n,
      sessionId: "session-13",
      streamStatus: "ready",
    });
  });

  it("rehydrates query-owned approvals when main requests reconnect recovery", async () => {
    const stream = createFakeSubscription();
    const listApprovals = vi
      .fn<() => Promise<ApprovalSnapshotResult>>()
      .mockResolvedValueOnce(
        makeApprovalSnapshot([makeApprovalRequest("approval-1", "old reason")], 10n),
      )
      .mockResolvedValueOnce(
        makeApprovalSnapshot([makeApprovalRequest("approval-2", "new reason")], 20n),
      );
    const hydrateSnapshot = vi.fn();
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {}),
        hydrateSnapshot,
        listApprovals: vi.fn(() => listApprovals()),
        subscribeApprovalStream: vi.fn(stream.subscribeApprovalStream),
      },
      sessionId: "session-10",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit({ stream: "approvals", status: "historyGap" });
    await flushMicrotasks();

    expect(listApprovals).toHaveBeenCalledTimes(2);
    expect(hydrateSnapshot).toHaveBeenLastCalledWith(
      makeApprovalSnapshot([makeApprovalRequest("approval-2", "new reason")], 20n),
    );
  });

  it("marks the approval stream errored on terminal status envelopes", async () => {
    const stream = createFakeSubscription();
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {}),
        hydrateSnapshot: vi.fn(),
        listApprovals: vi.fn(async () =>
          makeApprovalSnapshot([makeApprovalRequest("approval-1", "need shell")], 12n),
        ),
        subscribeApprovalStream: vi.fn(stream.subscribeApprovalStream),
      },
      sessionId: "session-12",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit({ stream: "approvals", status: "terminalError" });
    await flushMicrotasks();

    expect(actor.getSnapshot().context).toMatchObject({
      errorMessage: "approval stream entered a terminal error state for session-12",
      lastSequence: 12n,
      sessionId: "session-12",
      streamStatus: "error",
    });
  });

  it("automatically retries after a stream failure", async () => {
    vi.useFakeTimers();
    try {
      const openApprovalStream = vi
        .fn<SessionApprovalActorDeps["subscribeApprovalStream"]>()
        .mockRejectedValueOnce(new Error("socket down"))
        .mockResolvedValueOnce(() => undefined);
      const listApprovals = vi
        .fn<() => Promise<ApprovalSnapshotResult>>()
        .mockResolvedValue(makeApprovalSnapshot([], null));
      const actor = createSessionApprovalActor({
        deps: {
          decideApproval: vi.fn(async () => {}),
          hydrateSnapshot: vi.fn(),
          listApprovals: vi.fn(() => listApprovals()),
          subscribeApprovalStream: vi.fn((sessionId, afterCursor, onMessage, onError) =>
            openApprovalStream(sessionId, afterCursor, onMessage, onError),
          ),
        },
        sessionId: "session-retry",
      });

      actor.start();
      await flushMicrotasks();

      expect(actor.getSnapshot().context.streamStatus).toBe("error");

      await vi.advanceTimersByTimeAsync(1_500);
      await flushMicrotasks();

      expect(listApprovals).toHaveBeenCalledTimes(2);
      expect(openApprovalStream).toHaveBeenCalledTimes(2);
      expect(actor.getSnapshot().context.streamStatus).toBe("ready");

      actor.stop();
    } finally {
      vi.useRealTimers();
    }
  });
});
