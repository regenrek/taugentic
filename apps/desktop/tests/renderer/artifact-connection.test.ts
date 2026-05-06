import { describe, expect, it, vi } from "vite-plus/test";

import type { ArtifactSnapshotResult } from "../../packages/shared/generated/ArtifactSnapshotResult.js";
import type { ArtifactSummary } from "../../packages/shared/generated/ArtifactSummary.js";
import type {
  ArtifactStreamEventEnvelope,
  ArtifactStreamMessage,
} from "../../packages/shared/src/ipc.js";
import {
  createSessionArtifactActor,
  type SessionArtifactActorDeps,
} from "../../packages/renderer/src/features/artifacts/connection.js";

function makeArtifact(id: string, runId = "run-1"): ArtifactSummary {
  return {
    id,
    kind: "Patch",
    runId,
    storagePath: `artifacts/${runId}/${id}.diff`,
  };
}

function makeArtifactEvent(sequence: bigint): ArtifactStreamEventEnvelope {
  return {
    daemonInstanceId: "daemon-1",
    sessionId: "session-4",
    occurredAtMs: sequence * 10n,
    sequence,
    event: {
      artifact: {
        artifact: makeArtifact(`artifact-${sequence.toString()}`),
      },
    },
  };
}

function makeArtifactSnapshot(
  items: ArtifactSummary[],
  sequence: bigint | null = null,
): ArtifactSnapshotResult {
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
  let onMessage: ((message: ArtifactStreamMessage) => void) | undefined;
  const unsubscribe = vi.fn();

  return {
    emit(message: ArtifactStreamMessage) {
      onMessage?.(message);
    },
    fail(message: string) {
      onError?.(new Error(message));
    },
    subscribeArtifactStream: vi.fn(
      async (
        _sessionId: string,
        _afterCursor: ArtifactSnapshotResult["latestCursor"],
        nextOnMessage: (message: ArtifactStreamMessage) => void,
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

describe("artifact connection", () => {
  it("hydrates query-owned artifacts before opening the artifact live invalidator", async () => {
    const stream = createFakeSubscription();
    const snapshot = makeArtifactSnapshot([makeArtifact("artifact-1")], 12n);
    const listArtifacts = vi.fn(async () => snapshot);
    const hydrateSnapshot = vi.fn();
    const openArtifactStream = vi.fn(stream.subscribeArtifactStream);
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot,
        listArtifacts,
        subscribeArtifactStream: openArtifactStream,
      },
      sessionId: "session-1",
    });

    actor.start();
    await flushMicrotasks();

    expect(hydrateSnapshot).toHaveBeenCalledWith(snapshot);
    expect(actor.getSnapshot().context).toMatchObject({
      currentArtifactId: null,
      errorMessage: null,
      isHydrating: false,
      sessionId: "session-1",
      streamStatus: "live",
    });
    expect(openArtifactStream).toHaveBeenCalledWith(
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

  it("does not open the artifact live invalidator when artifact hydration fails", async () => {
    const stream = createFakeSubscription();
    const openArtifactStream = vi.fn(stream.subscribeArtifactStream);
    const onMissingSession = vi.fn<(sessionId: string) => void>();
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot: vi.fn(),
        listArtifacts: vi.fn(async () => {
          throw new Error("artifact read failed");
        }),
        subscribeArtifactStream: openArtifactStream,
      },
      onMissingSession,
      sessionId: "session-9",
    });

    actor.start();
    await flushMicrotasks();

    expect(openArtifactStream).not.toHaveBeenCalled();
    expect(onMissingSession).not.toHaveBeenCalled();
    expect(actor.getSnapshot().context).toMatchObject({
      currentArtifactId: null,
      errorMessage: "artifact refresh failed for session-9: artifact read failed",
      isHydrating: false,
      sessionId: "session-9",
      streamStatus: "error",
    });
  });

  it("signals the session owner when artifact hydration hits a missing session", async () => {
    const onMissingSession = vi.fn<(sessionId: string) => void>();
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot: vi.fn(),
        listArtifacts: vi.fn(async () => {
          throw new Error(
            "Error invoking remote method 'desktop:list-artifacts': DaemonJsonRpcError: daemon JSON-RPC error -32602: session does not exist: session-3",
          );
        }),
        subscribeArtifactStream: vi.fn(createFakeSubscription().subscribeArtifactStream),
      },
      onMissingSession,
      sessionId: "session-3",
    });

    actor.start();
    await flushMicrotasks();

    expect(onMissingSession).toHaveBeenCalledWith("session-3");
  });

  it("cleans up a late artifact subscription when the view unmounts during subscribe", async () => {
    const openArtifactStream = createDeferred<() => void>();
    const unsubscribe = vi.fn();
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot: vi.fn(),
        listArtifacts: vi.fn(async () => makeArtifactSnapshot([], null)),
        subscribeArtifactStream: vi.fn(() => openArtifactStream.promise),
      },
      sessionId: "session-7",
    });

    actor.start();
    await flushMicrotasks();

    actor.stop();
    openArtifactStream.resolve(unsubscribe);
    await flushMicrotasks();

    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("passes null as the first cursor when hydration has no daemon cursor yet", async () => {
    const stream = createFakeSubscription();
    const openArtifactStream = vi.fn(stream.subscribeArtifactStream);
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot: vi.fn(),
        listArtifacts: vi.fn(async () => makeArtifactSnapshot([], null)),
        subscribeArtifactStream: openArtifactStream,
      },
      sessionId: "session-11",
    });

    actor.start();
    await flushMicrotasks();

    expect(openArtifactStream).toHaveBeenCalledWith(
      "session-11",
      null,
      expect.any(Function),
      expect.any(Function),
    );
  });

  it("tolerates no-op artifact unsubscribers during teardown", async () => {
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot: vi.fn(),
        listArtifacts: vi.fn(async () => makeArtifactSnapshot([], null)),
        subscribeArtifactStream: vi.fn(async () => () => undefined),
      },
      sessionId: "session-8",
    });

    actor.start();
    await flushMicrotasks();

    expect(() => actor.stop()).not.toThrow();
  });

  it("refreshes query-owned artifacts whenever a live artifact event arrives", async () => {
    const stream = createFakeSubscription();
    const listArtifacts = vi
      .fn<() => Promise<ArtifactSnapshotResult>>()
      .mockResolvedValueOnce(makeArtifactSnapshot([makeArtifact("artifact-1")], 8n))
      .mockResolvedValueOnce(makeArtifactSnapshot([makeArtifact("artifact-2")], 9n));
    const hydrateSnapshot = vi.fn();
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot,
        listArtifacts: vi.fn(() => listArtifacts()),
        subscribeArtifactStream: vi.fn(stream.subscribeArtifactStream),
      },
      sessionId: "session-4",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit(makeArtifactEvent(9n));
    await flushMicrotasks();

    expect(listArtifacts).toHaveBeenCalledTimes(2);
    expect(hydrateSnapshot).toHaveBeenLastCalledWith(
      makeArtifactSnapshot([makeArtifact("artifact-2")], 9n),
    );
    expect(actor.getSnapshot().context).toMatchObject({
      currentArtifactId: null,
      errorMessage: null,
      isHydrating: false,
      sessionId: "session-4",
      streamStatus: "live",
    });
  });

  it("queues a trailing artifact refresh when a second live event lands during an in-flight refresh", async () => {
    const stream = createFakeSubscription();
    const deferredSnapshot = createDeferred<ArtifactSnapshotResult>();
    const listArtifacts = vi
      .fn<() => Promise<ArtifactSnapshotResult>>()
      .mockResolvedValueOnce(makeArtifactSnapshot([makeArtifact("artifact-1")], 8n))
      .mockImplementationOnce(() => deferredSnapshot.promise)
      .mockResolvedValueOnce(makeArtifactSnapshot([makeArtifact("artifact-2")], 10n));
    const hydrateSnapshot = vi.fn();
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot,
        listArtifacts: vi.fn(() => listArtifacts()),
        subscribeArtifactStream: vi.fn(stream.subscribeArtifactStream),
      },
      sessionId: "session-13",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit(makeArtifactEvent(9n));
    await flushMicrotasks();
    stream.emit(makeArtifactEvent(10n));
    await flushMicrotasks();
    deferredSnapshot.resolve(makeArtifactSnapshot([makeArtifact("artifact-1", "run-2")], 9n));
    await flushMicrotasks();

    expect(listArtifacts).toHaveBeenCalledTimes(3);
    expect(hydrateSnapshot).toHaveBeenLastCalledWith(
      makeArtifactSnapshot([makeArtifact("artifact-2")], 10n),
    );
  });

  it("rehydrates query-owned artifacts when main requests reconnect recovery", async () => {
    const stream = createFakeSubscription();
    const listArtifacts = vi
      .fn<() => Promise<ArtifactSnapshotResult>>()
      .mockResolvedValueOnce(makeArtifactSnapshot([makeArtifact("artifact-1")], 8n))
      .mockResolvedValueOnce(makeArtifactSnapshot([makeArtifact("artifact-2")], 9n));
    const hydrateSnapshot = vi.fn();
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot,
        listArtifacts: vi.fn(() => listArtifacts()),
        subscribeArtifactStream: vi.fn(stream.subscribeArtifactStream),
      },
      sessionId: "session-10",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit({ stream: "artifacts", status: "historyGap" });
    await flushMicrotasks();

    expect(listArtifacts).toHaveBeenCalledTimes(2);
    expect(hydrateSnapshot).toHaveBeenLastCalledWith(
      makeArtifactSnapshot([makeArtifact("artifact-2")], 9n),
    );
  });

  it("marks the artifact stream errored on terminal status envelopes", async () => {
    const stream = createFakeSubscription();
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot: vi.fn(),
        listArtifacts: vi.fn(async () => makeArtifactSnapshot([makeArtifact("artifact-1")], 12n)),
        subscribeArtifactStream: vi.fn(stream.subscribeArtifactStream),
      },
      sessionId: "session-12",
    });

    actor.start();
    await flushMicrotasks();
    stream.emit({ stream: "artifacts", status: "terminalError" });
    await flushMicrotasks();

    expect(actor.getSnapshot().context).toMatchObject({
      currentArtifactId: null,
      errorMessage: "artifact stream entered a terminal error state for session-12",
      isHydrating: false,
      sessionId: "session-12",
      streamStatus: "error",
    });
  });

  it("keeps the selected artifact id when refresh still returns that artifact", async () => {
    const stream = createFakeSubscription();
    const listArtifacts = vi
      .fn<() => Promise<ArtifactSnapshotResult>>()
      .mockResolvedValueOnce(makeArtifactSnapshot([makeArtifact("artifact-1")], 9n))
      .mockResolvedValueOnce(
        makeArtifactSnapshot([makeArtifact("artifact-1"), makeArtifact("artifact-2")], 10n),
      );
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot: vi.fn(),
        listArtifacts: vi.fn(() => listArtifacts()),
        subscribeArtifactStream: vi.fn(stream.subscribeArtifactStream),
      },
      sessionId: "session-5",
    });

    actor.start();
    await flushMicrotasks();
    actor.send({ type: "artifactSelected", artifactId: "artifact-1" });
    stream.emit(makeArtifactEvent(10n));
    await flushMicrotasks();

    expect(actor.getSnapshot().context.currentArtifactId).toBe("artifact-1");
  });

  it("keeps the last good artifact selection visible when a later refresh fails", async () => {
    const stream = createFakeSubscription();
    const listArtifacts = vi
      .fn<() => Promise<ArtifactSnapshotResult>>()
      .mockResolvedValueOnce(makeArtifactSnapshot([makeArtifact("artifact-1")], 10n))
      .mockRejectedValueOnce(new Error("refresh failed"));
    const actor = createSessionArtifactActor({
      deps: {
        hydrateSnapshot: vi.fn(),
        listArtifacts: vi.fn(() => listArtifacts()),
        subscribeArtifactStream: vi.fn(stream.subscribeArtifactStream),
      },
      sessionId: "session-6",
    });

    actor.start();
    await flushMicrotasks();
    actor.send({ type: "artifactSelected", artifactId: "artifact-1" });
    stream.emit(makeArtifactEvent(11n));
    await flushMicrotasks();

    expect(actor.getSnapshot().context).toMatchObject({
      currentArtifactId: "artifact-1",
      errorMessage: "artifact refresh failed for session-6: refresh failed",
      isHydrating: false,
      sessionId: "session-6",
      streamStatus: "error",
    });
  });

  it("automatically retries after a stream failure", async () => {
    vi.useFakeTimers();
    try {
      const openArtifactStream = vi
        .fn<SessionArtifactActorDeps["subscribeArtifactStream"]>()
        .mockRejectedValueOnce(new Error("socket down"))
        .mockResolvedValueOnce(() => undefined);
      const listArtifacts = vi
        .fn<() => Promise<ArtifactSnapshotResult>>()
        .mockResolvedValue(makeArtifactSnapshot([], null));
      const actor = createSessionArtifactActor({
        deps: {
          hydrateSnapshot: vi.fn(),
          listArtifacts: vi.fn(() => listArtifacts()),
          subscribeArtifactStream: vi.fn((sessionId, afterCursor, onMessage, onError) =>
            openArtifactStream(sessionId, afterCursor, onMessage, onError),
          ),
        },
        sessionId: "session-retry",
      });

      actor.start();
      await flushMicrotasks();

      expect(actor.getSnapshot().context.streamStatus).toBe("error");

      await vi.advanceTimersByTimeAsync(1_500);
      await flushMicrotasks();

      expect(listArtifacts).toHaveBeenCalledTimes(2);
      expect(openArtifactStream).toHaveBeenCalledTimes(2);
      expect(actor.getSnapshot().context.streamStatus).toBe("live");

      actor.stop();
    } finally {
      vi.useRealTimers();
    }
  });
});
