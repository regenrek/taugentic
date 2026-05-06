import type {
  ApprovalDecision,
  ApprovalId,
  ApprovalSnapshotResult,
  ApprovalStreamMessage,
  DaemonEventCursor,
  ListApprovalsQuery,
  SessionId,
} from "@taugentic/desktop-shared";
import { assign, createActor, fromCallback, fromPromise, setup, type SnapshotFrom } from "xstate";

import { decideApproval, listApprovals } from "../../lib/ipc/api";
import { subscribeApprovalStream, type StreamUnsubscribe } from "../../lib/ipc/stream";
import { isMissingSessionError } from "../sessions/selection";
import {
  createInitialSessionApprovalState,
  reduceApprovalStreamMessage,
  toApprovalStreamErrorMessage,
  type SessionApprovalState,
} from "./stream-state";

export interface SessionApprovalActorDeps {
  decideApproval(
    sessionId: SessionId,
    approvalId: ApprovalId,
    decision: ApprovalDecision,
  ): Promise<void>;
  hydrateSnapshot(snapshot: ApprovalSnapshotResult): void;
  listApprovals(sessionId: SessionId, query: ListApprovalsQuery): Promise<ApprovalSnapshotResult>;
  subscribeApprovalStream(
    sessionId: SessionId,
    afterCursor: DaemonEventCursor | null,
    onMessage: (message: ApprovalStreamMessage) => void,
    onError?: (error: Error) => void,
  ): Promise<StreamUnsubscribe>;
}

export interface SessionApprovalMachineInput {
  deps?: SessionApprovalActorDeps;
  onMissingSession?: (sessionId: SessionId) => void;
  sessionId: SessionId;
}

interface SessionApprovalContext extends SessionApprovalState {
  commandErrorMessage: string | null;
  deps: SessionApprovalActorDeps;
  latestCursor: DaemonEventCursor | null;
  onMissingSession?: (sessionId: SessionId) => void;
  pendingApprovalId: ApprovalId | null;
  pendingDecision: ApprovalDecision | null;
  refreshQueued: boolean;
}

type SessionApprovalMachineEvent =
  | {
      type: "approvalDecisionRequested";
      approvalId: ApprovalId;
      decision: ApprovalDecision;
    }
  | {
      type: "streamFailed";
      message: string;
    }
  | {
      type: "streamEnvelopeReceived";
      message: ApprovalStreamMessage;
    };

const STREAM_RETRY_DELAY_MS = 1_500;

const defaultDeps: SessionApprovalActorDeps = {
  async decideApproval(sessionId, approvalId, decision) {
    await decideApproval(sessionId, approvalId, decision);
  },
  hydrateSnapshot() {},
  async listApprovals(sessionId, query) {
    return listApprovals(sessionId, query);
  },
  async subscribeApprovalStream(sessionId, afterCursor, onMessage, onError) {
    return subscribeApprovalStream(sessionId, afterCursor, onMessage, onError);
  },
};

export const sessionApprovalMachine = setup({
  types: {
    context: {} as SessionApprovalContext,
    events: {} as SessionApprovalMachineEvent,
    input: {} as SessionApprovalMachineInput,
  },
  actors: {
    loadApprovals: fromPromise<
      ApprovalSnapshotResult,
      { deps: SessionApprovalActorDeps; sessionId: SessionId }
    >(async ({ input }) => input.deps.listApprovals(input.sessionId, {})),
    subscribeApprovalStream: fromCallback<
      SessionApprovalMachineEvent,
      {
        afterCursor: DaemonEventCursor | null;
        deps: SessionApprovalActorDeps;
        sessionId: SessionId;
      }
    >(({ input, sendBack }) => {
      let disposed = false;
      let unsubscribeStream: StreamUnsubscribe | null = null;
      void input.deps
        .subscribeApprovalStream(
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
            message: { stream: "approvals", status: "ready" },
          });
        })
        .catch((error: unknown) => {
          if (disposed) {
            return;
          }
          sendBack({
            type: "streamFailed",
            message: toApprovalStreamErrorMessage(input.sessionId, error),
          });
        });

      return () => {
        disposed = true;
        unsubscribeStream?.();
      };
    }),
    submitApprovalDecision: fromPromise<
      void,
      {
        approvalId: ApprovalId;
        decision: ApprovalDecision;
        deps: SessionApprovalActorDeps;
        sessionId: SessionId;
      }
    >(async ({ input }) =>
      input.deps.decideApproval(input.sessionId, input.approvalId, input.decision),
    ),
  },
  actions: {
    prepareRetry: assign({
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
    approvalMessageNeedsRefresh: ({ context, event }) =>
      event.type === "streamEnvelopeReceived" &&
      reduceApprovalContextWithMessage(context, event.message).needsRefresh,
    hasQueuedRefresh: ({ context }) => context.refreshQueued,
  },
}).createMachine({
  id: "sessionApproval",
  type: "parallel",
  context: ({ input }) => ({
    ...createInitialSessionApprovalState(input.sessionId),
    commandErrorMessage: null,
    deps: input.deps ?? defaultDeps,
    latestCursor: null,
    onMissingSession: input.onMissingSession,
    pendingApprovalId: null,
    pendingDecision: null,
    refreshQueued: false,
  }),
  states: {
    command: {
      initial: "idle",
      states: {
        idle: {
          on: {
            approvalDecisionRequested: {
              actions: assign(({ event }) => ({
                commandErrorMessage: null,
                pendingApprovalId: event.approvalId,
                pendingDecision: event.decision,
              })),
              target: "pending",
            },
          },
        },
        pending: {
          invoke: {
            input: ({ context }) => ({
              approvalId: context.pendingApprovalId as ApprovalId,
              decision: context.pendingDecision as ApprovalDecision,
              deps: context.deps,
              sessionId: context.sessionId,
            }),
            onDone: {
              actions: assign({
                commandErrorMessage: () => null,
                pendingApprovalId: () => null,
                pendingDecision: () => null,
              }),
              target: "idle",
            },
            onError: {
              actions: assign(({ event }) => ({
                commandErrorMessage: toErrorMessage(event.error),
                pendingApprovalId: null,
                pendingDecision: null,
              })),
              target: "idle",
            },
            src: "submitApprovalDecision",
          },
          on: {
            approvalDecisionRequested: {
              actions: assign(({ event }) => ({
                commandErrorMessage: null,
                pendingApprovalId: event.approvalId,
                pendingDecision: event.decision,
              })),
              reenter: true,
              target: "pending",
            },
          },
        },
      },
    },
    stream: {
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
                ...hydrateApprovalSnapshot(context, event.output),
                refreshQueued: false,
              })),
              target: "connecting",
            },
            onError: {
              actions: [
                "notifyMissingSessionIfNeeded",
                assign(({ context, event }) => toApprovalRefreshError(context, event.error)),
              ],
              target: "failed",
            },
            src: "loadApprovals",
          },
        },
        live: {
          invoke: {
            input: ({ context }) => ({
              afterCursor: context.latestCursor,
              deps: context.deps,
              sessionId: context.sessionId,
            }),
            src: "subscribeApprovalStream",
          },
          on: {
            streamFailed: {
              target: "#sessionApproval.stream.failed",
              actions: assign(({ event }) => ({
                errorMessage: event.type === "streamFailed" ? event.message : null,
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
                      ...reduceApprovalContextWithMessage(context, event.message).updates,
                      refreshQueued: false,
                    })),
                    guard: "approvalMessageNeedsRefresh",
                    target: "refreshing",
                  },
                  {
                    actions: assign(
                      ({ context, event }) =>
                        reduceApprovalContextWithMessage(context, event.message).updates,
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
                      ...hydrateApprovalSnapshot(context, event.output),
                      refreshQueued: false,
                    })),
                    guard: "hasQueuedRefresh",
                    reenter: true,
                    target: "refreshing",
                  },
                  {
                    actions: assign(({ context, event }) => ({
                      ...hydrateApprovalSnapshot(context, event.output),
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
                        ...toApprovalRefreshError(context, event.error),
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
                        ...toApprovalRefreshError(context, event.error),
                        refreshQueued: false,
                      })),
                    ],
                    target: "idle",
                  },
                ],
                src: "loadApprovals",
              },
              on: {
                streamEnvelopeReceived: [
                  {
                    actions: assign(({ context, event }) => ({
                      ...reduceApprovalContextWithMessage(context, event.message).updates,
                      refreshQueued: true,
                    })),
                    guard: "approvalMessageNeedsRefresh",
                  },
                  {
                    actions: assign(
                      ({ context, event }) =>
                        reduceApprovalContextWithMessage(context, event.message).updates,
                    ),
                  },
                ],
              },
            },
          },
        },
      },
    },
  },
});

export type SessionApprovalSnapshot = SnapshotFrom<typeof sessionApprovalMachine>;

export function createSessionApprovalActor(input: SessionApprovalMachineInput) {
  return createActor(sessionApprovalMachine, { input });
}

function toApprovalRefreshError(
  context: SessionApprovalContext,
  error: unknown,
): Pick<SessionApprovalContext, "errorMessage" | "streamStatus"> {
  return {
    errorMessage: toApprovalStreamErrorMessage(context.sessionId, error),
    streamStatus: "error",
  };
}

function hydrateApprovalSnapshot(
  context: SessionApprovalContext,
  snapshot: ApprovalSnapshotResult,
): Pick<SessionApprovalContext, "errorMessage" | "lastSequence" | "latestCursor" | "streamStatus"> {
  context.deps.hydrateSnapshot(snapshot);
  return {
    errorMessage: null,
    lastSequence: snapshot.latestCursor?.sequence ?? context.lastSequence,
    latestCursor: snapshot.latestCursor ?? null,
    streamStatus: context.streamStatus === "error" ? "connecting" : context.streamStatus,
  };
}

function reduceApprovalContextWithMessage(
  context: SessionApprovalContext,
  message: ApprovalStreamMessage,
): {
  needsRefresh: boolean;
  updates: Pick<SessionApprovalContext, "errorMessage" | "lastSequence" | "streamStatus">;
} {
  const reduced = reduceApprovalStreamMessage(toSessionApprovalState(context), message);
  return {
    needsRefresh: reduced.needsRefresh,
    updates: {
      errorMessage: reduced.state.errorMessage,
      lastSequence: reduced.state.lastSequence,
      streamStatus: reduced.state.streamStatus,
    },
  };
}

function toSessionApprovalState(context: SessionApprovalContext): SessionApprovalState {
  return {
    errorMessage: context.errorMessage,
    lastSequence: context.lastSequence,
    sessionId: context.sessionId,
    streamStatus: context.streamStatus,
  };
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
