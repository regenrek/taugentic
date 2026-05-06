import { assign, createActor, fromCallback, fromPromise, setup, type ActorRefFrom } from "xstate";

import type {
  AgentStreamMessage,
  AgentTurnsPageResult,
  DaemonEventCursor,
  SessionId,
} from "@taugentic/desktop-shared";

import type { StreamUnsubscribe } from "@/lib/ipc/stream";
import { DEFAULT_AGENT_TURNS_PAGE_LIMIT } from "@/lib/queries/session-queries";

import {
  clearLiveOverlay,
  createInitialSessionAgentStreamState,
  hydrateCommittedAgentTurns,
  reduceAgentStreamMessage,
  toAgentStreamErrorMessage,
  type SessionAgentStreamState,
} from "./state";

const STREAM_RETRY_DELAY_MS = 1_500;

export interface SessionAgentStreamDeps {
  loadCommitted(sessionId: SessionId, limit: number): Promise<AgentTurnsPageResult>;
  subscribeAgentStream(
    sessionId: SessionId,
    afterCursor: DaemonEventCursor | null,
    onMessage: (message: AgentStreamMessage) => void,
    onError?: (error: Error) => void,
  ): Promise<StreamUnsubscribe>;
}

interface SessionAgentStreamMachineInput {
  deps: SessionAgentStreamDeps;
  limit: number;
  sessionId: SessionId;
}

interface SessionAgentStreamContext extends SessionAgentStreamState {
  afterCursor: DaemonEventCursor | null;
  deps: SessionAgentStreamDeps;
  limit: number;
}

type SessionAgentStreamEvent =
  | { type: "committed.failed"; message: string }
  | { type: "committed.loaded"; snapshot: AgentTurnsPageResult }
  | { type: "stream.failed"; message: string }
  | { type: "stream.message"; message: AgentStreamMessage };

const sessionAgentStreamMachine = setup({
  types: {
    context: {} as SessionAgentStreamContext,
    events: {} as SessionAgentStreamEvent,
    input: {} as SessionAgentStreamMachineInput,
  },
  actors: {
    loadCommitted: fromPromise<
      AgentTurnsPageResult,
      {
        deps: SessionAgentStreamDeps;
        limit: number;
        sessionId: SessionId;
      }
    >(async ({ input }) => {
      return input.deps.loadCommitted(input.sessionId, input.limit);
    }),
    streamSubscription: fromCallback<
      SessionAgentStreamEvent,
      {
        afterCursor: DaemonEventCursor | null;
        deps: SessionAgentStreamDeps;
        limit: number;
        sessionId: SessionId;
      }
    >(({ input, sendBack }) => {
      let disposed = false;
      let unsubscribeStream: StreamUnsubscribe | null = null;
      let refreshInFlight = false;

      const triggerCommittedRefresh = () => {
        if (refreshInFlight) {
          return;
        }
        refreshInFlight = true;
        void input.deps
          .loadCommitted(input.sessionId, input.limit)
          .then((snapshot) => {
            if (disposed) {
              return;
            }
            sendBack({
              type: "committed.loaded",
              snapshot,
            });
          })
          .catch((error: unknown) => {
            if (disposed) {
              return;
            }
            const message = toAgentStreamErrorMessage(input.sessionId, error);
            if (typeof console !== "undefined" && typeof console.warn === "function") {
              console.warn(message);
            }
            sendBack({
              type: "committed.failed",
              message,
            });
          })
          .finally(() => {
            refreshInFlight = false;
          });
      };

      void input.deps
        .subscribeAgentStream(
          input.sessionId,
          input.afterCursor,
          (message) => {
            if (shouldInvalidateCommitted(message)) {
              triggerCommittedRefresh();
            }
            sendBack({
              type: "stream.message",
              message,
            });
          },
          (error) => {
            sendBack({
              type: "stream.failed",
              message: error.message,
            });
          },
        )
        .then((nextUnsubscribe) => {
          if (disposed) {
            nextUnsubscribe();
            return;
          }

          unsubscribeStream = nextUnsubscribe;
        })
        .catch((error: unknown) => {
          if (disposed) {
            return;
          }
          sendBack({
            type: "stream.failed",
            message: toAgentStreamErrorMessage(input.sessionId, error),
          });
        });

      return () => {
        disposed = true;
        unsubscribeStream?.();
      };
    }),
  },
  actions: {
    assignCursorFromMessage: assign(({ event }) => {
      if (event.type !== "stream.message") {
        return {};
      }
      const nextCursor = cursorFromMessage(event.message);
      if (nextCursor === undefined) {
        return {};
      }
      return {
        afterCursor: nextCursor,
      };
    }),
    assignHydrationError: assign(({ context, event }: any) => ({
      errorMessage: toAgentStreamErrorMessage(context.sessionId, event.error),
      streamStatus: "error" as const,
    })),
    assignStreamFailure: assign({
      errorMessage: ({ event }) => (event.type === "stream.failed" ? event.message : null),
      streamStatus: () => "error" as const,
    }),
    assignCommittedFailure: assign({
      errorMessage: ({ event }) => (event.type === "committed.failed" ? event.message : null),
    }),
    assignCommittedSnapshot: assign(({ context, event }) => {
      if (event.type !== "committed.loaded") {
        return {};
      }
      return hydrateCommittedAgentTurns(context, event.snapshot);
    }),
    assignStreamMessage: assign(({ context, event }) => {
      if (event.type !== "stream.message") {
        return {};
      }
      return reduceAgentStreamMessage(context, event.message).state;
    }),
    markRehydratingCommitted: assign(({ context }) => ({
      ...clearLiveOverlay(context),
      errorMessage: null,
      streamStatus: "rehydratingCommitted" as const,
    })),
    markReopeningLiveStream: assign({
      errorMessage: () => null,
      streamStatus: () => "reopeningLiveStream" as const,
    }),
    prepareRetry: assign({
      errorMessage: () => null,
      streamStatus: () => "connecting" as const,
    }),
  },
}).createMachine({
  id: "sessionAgentStream",
  initial: "hydratingCommitted",
  context: ({ input }) => ({
    ...createInitialSessionAgentStreamState(input.sessionId),
    afterCursor: null,
    deps: input.deps,
    limit: input.limit,
  }),
  states: {
    hydratingCommitted: {
      invoke: {
        src: "loadCommitted",
        input: ({ context }) => ({
          deps: context.deps,
          limit: context.limit,
          sessionId: context.sessionId,
        }),
        onDone: {
          actions: assign(({ context, event }) =>
            hydrateCommittedAgentTurns(context, event.output),
          ),
          target: "streaming",
        },
        onError: {
          actions: "assignHydrationError",
          target: "failed",
        },
      },
    },
    streaming: {
      on: {
        "committed.failed": {
          actions: "assignCommittedFailure",
        },
        "committed.loaded": {
          actions: "assignCommittedSnapshot",
        },
        "stream.failed": {
          actions: "assignStreamFailure",
          target: "failed",
        },
        "stream.message": [
          {
            guard: ({ event }) =>
              event.type === "stream.message" &&
              "status" in event.message &&
              event.message.status === "historyGap",
            actions: ["assignCursorFromMessage", "assignStreamMessage"],
            target: "recoveringFromGap",
          },
          {
            actions: ["assignCursorFromMessage", "assignStreamMessage"],
          },
        ],
      },
      invoke: {
        src: "streamSubscription",
        input: ({ context }) => ({
          afterCursor: context.afterCursor,
          deps: context.deps,
          limit: context.limit,
          sessionId: context.sessionId,
        }),
      },
    },
    recoveringFromGap: {
      always: {
        target: "revalidatingCommitted",
      },
    },
    revalidatingCommitted: {
      entry: "markRehydratingCommitted",
      invoke: {
        src: "loadCommitted",
        input: ({ context }) => ({
          deps: context.deps,
          limit: context.limit,
          sessionId: context.sessionId,
        }),
        onDone: {
          actions: [
            assign(({ context, event }) => hydrateCommittedAgentTurns(context, event.output)),
            "markReopeningLiveStream",
          ],
          target: "reopeningLiveStream",
        },
        onError: {
          actions: "assignHydrationError",
          target: "failed",
        },
      },
    },
    reopeningLiveStream: {
      on: {
        "committed.failed": {
          actions: "assignCommittedFailure",
        },
        "committed.loaded": {
          actions: "assignCommittedSnapshot",
        },
        "stream.failed": {
          actions: "assignStreamFailure",
          target: "failed",
        },
        "stream.message": [
          {
            guard: ({ event }) =>
              event.type === "stream.message" &&
              "status" in event.message &&
              event.message.status === "historyGap",
            actions: ["assignCursorFromMessage", "assignStreamMessage"],
            target: "recoveringFromGap",
          },
          {
            actions: ["assignCursorFromMessage", "assignStreamMessage"],
          },
        ],
      },
      invoke: {
        src: "streamSubscription",
        input: ({ context }) => ({
          afterCursor: context.afterCursor,
          deps: context.deps,
          limit: context.limit,
          sessionId: context.sessionId,
        }),
      },
    },
    failed: {
      after: {
        [STREAM_RETRY_DELAY_MS]: {
          actions: "prepareRetry",
          target: "hydratingCommitted",
        },
      },
    },
  },
});

interface AgentStreamSessionHandle {
  actorRef: ActorRefFrom<typeof sessionAgentStreamMachine>;
  refCount: number;
  sessionId: SessionId;
}

const sessionRegistry = new Map<SessionId, AgentStreamSessionHandle>();

function acquireSessionHandle(
  sessionId: SessionId,
  deps: SessionAgentStreamDeps,
  limit: number,
): AgentStreamSessionHandle {
  const existing = sessionRegistry.get(sessionId);
  if (existing) {
    existing.refCount += 1;
    return existing;
  }

  const actorRef = createActor(sessionAgentStreamMachine, {
    input: {
      deps,
      limit,
      sessionId,
    },
  });
  actorRef.start();
  const created: AgentStreamSessionHandle = {
    actorRef,
    refCount: 1,
    sessionId,
  };
  sessionRegistry.set(sessionId, created);
  return created;
}

function releaseSessionHandle(handle: AgentStreamSessionHandle): void {
  const current = sessionRegistry.get(handle.sessionId);
  if (!current) {
    return;
  }
  current.refCount -= 1;
  if (current.refCount > 0) {
    return;
  }
  sessionRegistry.delete(handle.sessionId);
  current.actorRef.stop();
}

export interface AgentStreamSessionHandleForTests {
  actorRef: ActorRefFrom<typeof sessionAgentStreamMachine>;
  sessionId: SessionId;
}

export function acquireAgentStreamSessionHandleForTests(
  sessionId: SessionId,
  deps: SessionAgentStreamDeps,
  limit: number = DEFAULT_AGENT_TURNS_PAGE_LIMIT,
): AgentStreamSessionHandleForTests {
  return acquireSessionHandle(sessionId, deps, limit);
}

export function releaseAgentStreamSessionHandleForTests(
  handle: AgentStreamSessionHandleForTests,
): void {
  releaseSessionHandle(handle as AgentStreamSessionHandle);
}

export function resetAgentStreamSessionRegistryForTests(): void {
  for (const handle of sessionRegistry.values()) {
    handle.actorRef.stop();
  }
  sessionRegistry.clear();
}

export function acquireAgentStreamSessionHandle(
  sessionId: SessionId,
  deps: SessionAgentStreamDeps,
  limit: number,
): AgentStreamSessionHandle {
  return acquireSessionHandle(sessionId, deps, limit);
}

export function releaseAgentStreamSessionHandle(handle: { sessionId: SessionId }): void {
  releaseSessionHandle(handle as AgentStreamSessionHandle);
}

function cursorFromMessage(message: AgentStreamMessage): DaemonEventCursor | null | undefined {
  if ("status" in message) {
    return message.latestCursor;
  }
  return {
    daemonInstanceId: message.daemonInstanceId,
    sessionId: message.sessionId,
    sequence: message.sequence,
  };
}

function shouldInvalidateCommitted(message: AgentStreamMessage): boolean {
  if ("status" in message || !("agentStream" in message.event)) {
    return false;
  }
  switch (message.event.agentStream.frame.kind) {
    case "assistantTurnCompleted":
    case "toolCallCompleted":
    case "pendingStateChanged":
    case "tokenUsageUpdated":
      return true;
    case "assistantTurnStarted":
    case "assistantMessageDelta":
    case "toolCallStarted":
    case "toolCallProgressed":
      return false;
  }
}
