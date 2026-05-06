import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import type { AgentTurnsPageResult, SessionId } from "../../packages/shared/generated/index.js";
import type { AgentStreamMessage } from "../../packages/shared/src/ipc.js";

import {
  acquireAgentStreamSessionHandleForTests,
  releaseAgentStreamSessionHandleForTests,
  resetAgentStreamSessionRegistryForTests,
  type SessionAgentStreamDeps,
} from "../../packages/renderer/src/features/agent-stream/index.js";

interface FakeSubscription<TMessage> {
  emit(message: TMessage): void;
}

function makeFakeSubscription<TMessage>() {
  let onMessage: ((message: TMessage) => void) | undefined;
  const unsubscribe = vi.fn();
  const fake = {
    emit(message: TMessage) {
      onMessage?.(message);
    },
    subscribe: vi.fn(
      async (
        _sessionId: SessionId,
        _afterCursor: unknown,
        nextOnMessage: (message: TMessage) => void,
      ) => {
        onMessage = nextOnMessage;
        return unsubscribe;
      },
    ),
    unsubscribe,
  };
  return fake;
}

function assistantRow(sessionId: SessionId, text: string) {
  return {
    kind: "assistant" as const,
    cursor: { sequence: 9n },
    sessionId,
    runId: "run-1",
    turnId: "turn-1",
    startedAtMs: 90n,
    completedAtMs: 95n,
    text,
  };
}

function emptyAgentTurnsPage(): AgentTurnsPageResult {
  return {
    items: [],
    latestCursor: null,
    nextBefore: null,
  };
}

function agentDeltaMessage(sessionId: SessionId, text: string): AgentStreamMessage {
  return {
    daemonInstanceId: "daemon-1",
    sessionId,
    sequence: 10n,
    occurredAtMs: 100n,
    event: {
      agentStream: {
        runId: "run-1",
        turnId: "turn-1",
        itemId: null,
        fragmentSequence: 1n,
        frame: {
          kind: "assistantMessageDelta",
          delta: text,
        },
      },
    },
  };
}

function assistantCompletedMessage(sessionId: SessionId): AgentStreamMessage {
  return {
    daemonInstanceId: "daemon-1",
    sessionId,
    sequence: 11n,
    occurredAtMs: 110n,
    event: {
      agentStream: {
        runId: "run-1",
        turnId: "turn-1",
        itemId: null,
        fragmentSequence: 2n,
        frame: {
          kind: "assistantTurnCompleted",
        },
      },
    },
  };
}

async function flushMicrotasks(): Promise<void> {
  for (let i = 0; i < 10; i += 1) {
    await Promise.resolve();
  }
}

afterEach(() => {
  resetAgentStreamSessionRegistryForTests();
});

describe("agent stream model", () => {
  it("hydrates one shared owner per session before opening the live stream", async () => {
    const sessionId = "session-1" as SessionId;
    const stream = makeFakeSubscription<AgentStreamMessage>();
    const loadCommitted = vi.fn(async () => ({
      items: [assistantRow(sessionId, "seeded durable row")],
      latestCursor: null,
      nextBefore: null,
    }));
    const subscribeAgentStream = vi.fn(stream.subscribe);
    const deps: SessionAgentStreamDeps = {
      loadCommitted,
      subscribeAgentStream,
    };

    const handleA = acquireAgentStreamSessionHandleForTests(sessionId, deps);
    const handleB = acquireAgentStreamSessionHandleForTests(sessionId, deps);

    await flushMicrotasks();

    expect(loadCommitted).toHaveBeenCalledTimes(1);
    expect(subscribeAgentStream).toHaveBeenCalledTimes(1);
    expect(handleA.actorRef).toBe(handleB.actorRef);
    expect(handleB.actorRef.getSnapshot().context.committedRows).toEqual([
      assistantRow(sessionId, "seeded durable row"),
    ]);
    expect(handleB.actorRef.getSnapshot().context.hasHydratedCommitted).toBe(true);

    stream.emit(agentDeltaMessage(sessionId, "shared text"));
    await flushMicrotasks();

    expect(handleB.actorRef.getSnapshot().context.liveMessages).toEqual([
      {
        completed: false,
        firstSequence: 10n,
        lastSequence: 10n,
        occurredAtMs: 100n,
        runId: "run-1",
        startedAtMs: 100n,
        text: "shared text",
        turnId: "turn-1",
      },
    ]);

    releaseAgentStreamSessionHandleForTests(handleA);
    releaseAgentStreamSessionHandleForTests(handleB);
  });

  it("repairs a completed assistant turn by rehydrating the same owner store", async () => {
    const sessionId = "session-1" as SessionId;
    const stream = makeFakeSubscription<AgentStreamMessage>();
    const loadCommitted = vi
      .fn()
      .mockResolvedValueOnce(emptyAgentTurnsPage())
      .mockResolvedValueOnce({
        items: [assistantRow(sessionId, "live streaming works")],
        latestCursor: null,
        nextBefore: null,
      });
    const deps: SessionAgentStreamDeps = {
      loadCommitted,
      subscribeAgentStream: vi.fn(stream.subscribe),
    };

    const handle = acquireAgentStreamSessionHandleForTests(sessionId, deps);
    await flushMicrotasks();

    expect(handle.actorRef.getSnapshot().context.committedRows).toEqual([]);

    stream.emit(assistantCompletedMessage(sessionId));
    await flushMicrotasks();

    expect(loadCommitted).toHaveBeenCalledTimes(2);
    expect(handle.actorRef.getSnapshot().context.committedRows).toEqual([
      assistantRow(sessionId, "live streaming works"),
    ]);
    expect(handle.actorRef.getSnapshot().context.liveMessages).toEqual([]);

    releaseAgentStreamSessionHandleForTests(handle);
  });

  it("clears live buffers, reloads committed rows, and reopens after historyGap", async () => {
    const sessionId = "session-gap" as SessionId;
    const streams: Array<FakeSubscription<AgentStreamMessage>> = [];
    const loadCommitted = vi
      .fn()
      .mockResolvedValueOnce(emptyAgentTurnsPage())
      .mockResolvedValueOnce({
        items: [assistantRow(sessionId, "rehydrated after gap")],
        latestCursor: null,
        nextBefore: null,
      });
    const subscribeAgentStream = vi.fn(async (_targetSessionId, _afterCursor, onMessage) => {
      const stream = makeFakeSubscription<AgentStreamMessage>();
      streams.push(stream);
      await stream.subscribe(sessionId, null, onMessage);
      return stream.unsubscribe;
    });
    const deps: SessionAgentStreamDeps = {
      loadCommitted,
      subscribeAgentStream,
    };

    const handle = acquireAgentStreamSessionHandleForTests(sessionId, deps);
    await flushMicrotasks();

    streams[0]!.emit(agentDeltaMessage(sessionId, "stale"));
    await flushMicrotasks();
    expect(handle.actorRef.getSnapshot().context.liveMessages).toHaveLength(1);

    streams[0]!.emit({
      stream: "agentStream",
      status: "historyGap",
      latestCursor: {
        daemonInstanceId: "daemon-1",
        sessionId,
        sequence: 25n,
      },
    });
    await flushMicrotasks();

    expect(loadCommitted).toHaveBeenCalledTimes(2);
    expect(subscribeAgentStream).toHaveBeenCalledTimes(2);
    expect(subscribeAgentStream).toHaveBeenNthCalledWith(
      2,
      sessionId,
      {
        daemonInstanceId: "daemon-1",
        sessionId,
        sequence: 25n,
      },
      expect.any(Function),
      expect.any(Function),
    );
    expect(handle.actorRef.getSnapshot().context.committedRows).toEqual([
      assistantRow(sessionId, "rehydrated after gap"),
    ]);
    expect(handle.actorRef.getSnapshot().context.liveMessages).toEqual([]);

    releaseAgentStreamSessionHandleForTests(handle);
  });
});
