import type {
  ArtifactId,
  ArtifactSnapshotResult,
  ArtifactStreamMessage,
  DaemonEventCursor,
  ListArtifactsQuery,
  SessionId,
} from "@taugentic/desktop-shared";
import { assign, createActor, fromCallback, fromPromise, setup } from "xstate";

import { listArtifacts } from "../../lib/ipc/api";
import { subscribeArtifactStream, type StreamUnsubscribe } from "../../lib/ipc/stream";
import { isMissingSessionError } from "../sessions/selection";
import { reconcileCurrentArtifactId } from "./selection";
import {
  createInitialSessionArtifactState,
  selectCurrentArtifact,
  toArtifactStreamErrorMessage,
  reduceArtifactStreamMessage,
  type SessionArtifactState,
} from "./state";

export interface SessionArtifactActorDeps {
  hydrateSnapshot(snapshot: ArtifactSnapshotResult): void;
  listArtifacts(sessionId: SessionId, query: ListArtifactsQuery): Promise<ArtifactSnapshotResult>;
  subscribeArtifactStream(
    sessionId: SessionId,
    afterCursor: DaemonEventCursor | null,
    onMessage: (message: ArtifactStreamMessage) => void,
    onError?: (error: Error) => void,
  ): Promise<StreamUnsubscribe>;
}

export interface SessionArtifactMachineInput {
  deps?: SessionArtifactActorDeps;
  onMissingSession?: (sessionId: SessionId) => void;
  sessionId: SessionId;
}

interface SessionArtifactContext extends SessionArtifactState {
  deps: SessionArtifactActorDeps;
  latestCursor: DaemonEventCursor | null;
  onMissingSession?: (sessionId: SessionId) => void;
  refreshQueued: boolean;
}

type SessionArtifactMachineEvent =
  | {
      type: "artifactSelected";
      artifactId: ArtifactId;
    }
  | {
      type: "streamFailed";
      message: string;
    }
  | {
      type: "streamEnvelopeReceived";
      message: ArtifactStreamMessage;
    };

const STREAM_RETRY_DELAY_MS = 1_500;

const defaultDeps: SessionArtifactActorDeps = {
  hydrateSnapshot() {},
  async listArtifacts(sessionId, query) {
    return listArtifacts(sessionId, query);
  },
  async subscribeArtifactStream(sessionId, afterCursor, onMessage, onError) {
    return subscribeArtifactStream(sessionId, afterCursor, onMessage, onError);
  },
};

export const sessionArtifactMachine = setup({
  types: {
    context: {} as SessionArtifactContext,
    events: {} as SessionArtifactMachineEvent,
    input: {} as SessionArtifactMachineInput,
  },
  actors: {
    loadArtifacts: fromPromise<
      ArtifactSnapshotResult,
      { deps: SessionArtifactActorDeps; sessionId: SessionId }
    >(async ({ input }) => input.deps.listArtifacts(input.sessionId, {})),
    subscribeArtifactStream: fromCallback<
      SessionArtifactMachineEvent,
      {
        afterCursor: DaemonEventCursor | null;
        deps: SessionArtifactActorDeps;
        sessionId: SessionId;
      }
    >(({ input, sendBack }) => {
      let disposed = false;
      let unsubscribeStream: StreamUnsubscribe | null = null;
      void input.deps
        .subscribeArtifactStream(
          input.sessionId,
          input.afterCursor,
          (message) => {
            sendBack({ type: "streamEnvelopeReceived", message });
          },
          (error) => {
            sendBack({
              type: "streamFailed",
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
          sendBack({
            type: "streamEnvelopeReceived",
            message: { stream: "artifacts", status: "ready" },
          });
        })
        .catch((error: unknown) => {
          if (disposed) {
            return;
          }
          sendBack({
            type: "streamFailed",
            message: toArtifactStreamErrorMessage(input.sessionId, error),
          });
        });

      return () => {
        disposed = true;
        unsubscribeStream?.();
      };
    }),
  },
  actions: {
    prepareRetry: assign({
      isHydrating: () => true,
      refreshQueued: () => false,
      streamStatus: () => "connecting" as const,
    }),
    notifyMissingSessionIfNeeded: ({ context, event }) => {
      if ("error" in event && isMissingSessionError(event.error, context.sessionId)) {
        context.onMissingSession?.(context.sessionId);
      }
    },
  },
  guards: {
    artifactMessageNeedsRefresh: ({ context, event }) =>
      event.type === "streamEnvelopeReceived" &&
      reduceArtifactContextWithMessage(context, event.message).needsRefresh,
    hasQueuedRefresh: ({ context }) => context.refreshQueued,
  },
}).createMachine({
  id: "sessionArtifact",
  context: ({ input }) => ({
    ...createInitialSessionArtifactState(input.sessionId),
    deps: input.deps ?? defaultDeps,
    latestCursor: null,
    onMissingSession: input.onMissingSession,
    refreshQueued: false,
  }),
  on: {
    artifactSelected: {
      actions: assign(({ context, event }) => {
        const nextState = selectCurrentArtifact(toSessionArtifactState(context), event.artifactId);
        return {
          currentArtifactId: nextState.currentArtifactId,
        };
      }),
    },
  },
  initial: "hydrating",
  states: {
    connecting: {
      always: {
        target: "live.idle",
      },
    },
    failed: {
      after: {
        [STREAM_RETRY_DELAY_MS]: {
          target: "hydrating",
          actions: "prepareRetry",
        },
      },
    },
    hydrating: {
      invoke: {
        input: ({ context }) => ({
          deps: context.deps,
          sessionId: context.sessionId,
        }),
        onDone: {
          actions: assign(({ context, event }) => ({
            ...hydrateArtifactSnapshot(context, event.output),
            refreshQueued: false,
          })),
          target: "connecting",
        },
        onError: {
          actions: [
            "notifyMissingSessionIfNeeded",
            assign(({ context, event }) => toArtifactRefreshError(context, event.error)),
          ],
          target: "failed",
        },
        src: "loadArtifacts",
      },
    },
    live: {
      invoke: {
        input: ({ context }) => ({
          afterCursor: context.latestCursor,
          deps: context.deps,
          sessionId: context.sessionId,
        }),
        src: "subscribeArtifactStream",
      },
      on: {
        streamFailed: {
          target: "#sessionArtifact.failed",
          actions: assign(({ event }) => ({
            errorMessage: event.type === "streamFailed" ? event.message : null,
            isHydrating: false,
            streamStatus: "error" as const,
          })),
        },
      },
      initial: "idle",
      states: {
        idle: {
          on: {
            streamEnvelopeReceived: [
              {
                actions: assign(({ context, event }) => ({
                  ...reduceArtifactContextWithMessage(context, event.message).updates,
                  refreshQueued: false,
                })),
                guard: "artifactMessageNeedsRefresh",
                target: "refreshing",
              },
              {
                actions: assign(
                  ({ context, event }) =>
                    reduceArtifactContextWithMessage(context, event.message).updates,
                ),
              },
            ],
          },
        },
        refreshing: {
          invoke: {
            input: ({ context }) => ({
              deps: context.deps,
              sessionId: context.sessionId,
            }),
            onDone: [
              {
                actions: assign(({ context, event }) => ({
                  ...hydrateArtifactSnapshot(context, event.output),
                  refreshQueued: false,
                })),
                guard: "hasQueuedRefresh",
                reenter: true,
                target: "refreshing",
              },
              {
                actions: assign(({ context, event }) => ({
                  ...hydrateArtifactSnapshot(context, event.output),
                  refreshQueued: false,
                })),
                target: "idle",
              },
            ],
            onError: [
              {
                actions: [
                  "notifyMissingSessionIfNeeded",
                  assign(({ context, event }) => ({
                    ...toArtifactRefreshError(context, event.error),
                    refreshQueued: false,
                  })),
                ],
                guard: "hasQueuedRefresh",
                reenter: true,
                target: "refreshing",
              },
              {
                actions: [
                  "notifyMissingSessionIfNeeded",
                  assign(({ context, event }) => ({
                    ...toArtifactRefreshError(context, event.error),
                    refreshQueued: false,
                  })),
                ],
                target: "idle",
              },
            ],
            src: "loadArtifacts",
          },
          on: {
            streamEnvelopeReceived: [
              {
                actions: assign(({ context, event }) => ({
                  ...reduceArtifactContextWithMessage(context, event.message).updates,
                  refreshQueued: true,
                })),
                guard: "artifactMessageNeedsRefresh",
              },
              {
                actions: assign(
                  ({ context, event }) =>
                    reduceArtifactContextWithMessage(context, event.message).updates,
                ),
              },
            ],
          },
        },
      },
    },
  },
});

export function createSessionArtifactActor(input: SessionArtifactMachineInput) {
  return createActor(sessionArtifactMachine, { input });
}

function toArtifactRefreshError(
  context: SessionArtifactContext,
  error: unknown,
): Pick<SessionArtifactContext, "errorMessage" | "isHydrating" | "streamStatus"> {
  return {
    errorMessage: toArtifactStreamErrorMessage(context.sessionId, error),
    isHydrating: false,
    streamStatus: "error",
  };
}

function hydrateArtifactSnapshot(
  context: SessionArtifactContext,
  snapshot: ArtifactSnapshotResult,
): Pick<
  SessionArtifactContext,
  "currentArtifactId" | "errorMessage" | "isHydrating" | "latestCursor" | "streamStatus"
> {
  context.deps.hydrateSnapshot(snapshot);
  return {
    currentArtifactId: reconcileCurrentArtifactId(context.currentArtifactId, snapshot.items),
    errorMessage: null,
    isHydrating: false,
    latestCursor: snapshot.latestCursor ?? null,
    streamStatus: context.streamStatus === "error" ? "connecting" : context.streamStatus,
  };
}

function reduceArtifactContextWithMessage(
  context: SessionArtifactContext,
  message: ArtifactStreamMessage,
): {
  needsRefresh: boolean;
  updates: Pick<
    SessionArtifactContext,
    "currentArtifactId" | "errorMessage" | "isHydrating" | "streamStatus"
  >;
} {
  const reduced = reduceArtifactStreamMessage(toSessionArtifactState(context), message);
  return {
    needsRefresh: reduced.needsRefresh,
    updates: {
      currentArtifactId: reduced.state.currentArtifactId,
      errorMessage: reduced.state.errorMessage,
      isHydrating: reduced.state.isHydrating,
      streamStatus: reduced.state.streamStatus,
    },
  };
}

function toSessionArtifactState(context: SessionArtifactContext): SessionArtifactState {
  return {
    currentArtifactId: context.currentArtifactId,
    errorMessage: context.errorMessage,
    isHydrating: context.isHydrating,
    sessionId: context.sessionId,
    streamStatus: context.streamStatus,
  };
}
