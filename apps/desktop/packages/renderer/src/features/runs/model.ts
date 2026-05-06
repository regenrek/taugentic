import { useQueryClient } from "@tanstack/react-query";
import { useMachine } from "@xstate/react";
import { assign, createActor, fromCallback, fromPromise, setup } from "xstate";

import type { ActivityCursor, RunStreamMessage, SessionId } from "@taugentic/desktop-shared";

import { getActivityPage, listRuns, startRun } from "../../lib/ipc/api";
import { subscribeRunStream } from "../../lib/ipc/stream";
import { queryKeys } from "../../lib/queries/keys";
import { useSessionRunActivityQuery, useSessionRunsQuery } from "../../lib/queries/session-queries";
import { isMissingSessionError } from "../sessions/selection";
import {
  type SessionRunConnectionDeps,
  type RunSnapshotRefreshResult,
  RECENT_RUN_ACTIVITY_QUERY,
} from "./connection";
import type { RunActivityItem, SessionRunState } from "./state";
import {
  RECENT_RUN_ACTIVITY_LIMIT,
  createInitialSessionRunState,
  hydrateRunActivity,
  reduceRunStreamMessage,
  toRunStreamErrorMessage,
} from "./state";

export interface SessionRunsMachineDeps extends SessionRunConnectionDeps {
  startRun: (sessionId: SessionId, objective: string) => Promise<void>;
}

export interface UseSessionRunsModelOptions {
  deps?: SessionRunsMachineDeps;
  onRunStarted?: () => void;
  onSessionInvalid?: (sessionId: SessionId) => void;
}

interface SessionRunsMachineInput {
  deps: SessionRunsMachineDeps;
  onRunStarted?: () => void;
  onSessionInvalid?: (sessionId: SessionId) => void;
  sessionId: SessionId;
}

interface SessionRunsMachineContext extends SessionRunState {
  afterCursor: ActivityCursor | null;
  commandErrorMessage: string | null;
  deps: SessionRunsMachineDeps;
  draftObjective: string;
  onRunStarted?: () => void;
  onSessionInvalid?: (sessionId: SessionId) => void;
  refreshPending: boolean;
  sessionId: SessionId;
}

type SessionRunsMachineEvent =
  | { type: "draftChanged"; value: string }
  | { type: "startRequested" }
  | { type: "stream.failed"; message: string }
  | { type: "stream.message"; message: RunStreamMessage };

interface StartRunInput {
  deps: SessionRunsMachineDeps;
  objective: string;
  sessionId: SessionId;
}

interface RunStreamSubscriptionInput {
  afterCursor: ActivityCursor | null;
  deps: SessionRunsMachineDeps;
  sessionId: SessionId;
}

const DEFAULT_DRAFT_OBJECTIVE = "Ship app server hard cut";
const STREAM_RETRY_DELAY_MS = 1_500;

function createInitialSessionRunsContext(
  input: SessionRunsMachineInput,
): SessionRunsMachineContext {
  return {
    ...createInitialSessionRunState(),
    afterCursor: null,
    commandErrorMessage: null,
    deps: input.deps,
    draftObjective: DEFAULT_DRAFT_OBJECTIVE,
    onRunStarted: input.onRunStarted,
    onSessionInvalid: input.onSessionInvalid,
    refreshPending: false,
    sessionId: input.sessionId,
  };
}

function toStreamState(context: SessionRunsMachineContext): SessionRunState {
  return {
    errorMessage: context.errorMessage,
    isHydrating: context.isHydrating,
    streamStatus: context.streamStatus,
  };
}

function toContextProjection(
  context: SessionRunsMachineContext,
  nextState: SessionRunState,
): Pick<
  SessionRunsMachineContext,
  "afterCursor" | "errorMessage" | "isHydrating" | "streamStatus"
> {
  return {
    afterCursor: context.afterCursor,
    errorMessage: nextState.errorMessage,
    isHydrating: nextState.isHydrating,
    streamStatus: nextState.streamStatus,
  };
}

function reduceMessage(
  context: SessionRunsMachineContext,
  message: RunStreamMessage,
): { needsRefresh: boolean; next: SessionRunState } {
  const reduced = reduceRunStreamMessage(toStreamState(context), message);
  return {
    needsRefresh: reduced.needsRefresh,
    next: reduced.state,
  };
}

function hydrateSnapshotState(
  context: SessionRunsMachineContext,
  snapshot: RunSnapshotRefreshResult,
): SessionRunState {
  context.deps.hydrateSnapshot(snapshot);
  return {
    ...toStreamState(context),
    errorMessage: null,
    isHydrating: false,
  };
}

const sessionRunsMachine = setup({
  types: {
    context: {} as SessionRunsMachineContext,
    events: {} as SessionRunsMachineEvent,
    input: {} as SessionRunsMachineInput,
  },
  actors: {
    loadSnapshot: fromPromise<
      RunSnapshotRefreshResult,
      Pick<SessionRunsMachineInput, "deps" | "sessionId">
    >(async ({ input }) => input.deps.loadSnapshot(input.sessionId)),
    startRunRequest: fromPromise<void, StartRunInput>(async ({ input }) => {
      await input.deps.startRun(input.sessionId, input.objective);
    }),
    streamSubscription: fromCallback<
      | { type: "stream.failed"; message: string }
      | { type: "stream.message"; message: RunStreamMessage },
      RunStreamSubscriptionInput
    >(({ input, sendBack }) => {
      let disposed = false;
      let unsubscribeStream: (() => void) | null = null;

      void input.deps
        .subscribeRunStream(
          input.sessionId,
          input.afterCursor,
          (message) => {
            sendBack({
              type: "stream.message",
              message,
            });
          },
          () => {
            sendBack({
              type: "stream.failed",
              message: `run stream decode failed for ${input.sessionId}`,
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
            type: "stream.message",
            message: { stream: "runs", status: "ready" },
          });
        })
        .catch((error: unknown) => {
          if (disposed) {
            return;
          }

          sendBack({
            type: "stream.failed",
            message: toRunStreamErrorMessage(input.sessionId, error),
          });
        });

      return () => {
        disposed = true;
        unsubscribeStream?.();
      };
    }),
  },
  guards: {
    hasRunObjective: ({ context }) => context.draftObjective.trim().length > 0,
    refreshQueued: ({ context }) => context.refreshPending,
    streamMessageNeedsRefresh: ({ context, event }) =>
      event.type === "stream.message" && reduceMessage(context, event.message).needsRefresh,
  },
  actions: {
    updateDraftObjective: assign({
      draftObjective: ({ event }) =>
        event.type === "draftChanged" ? event.value : DEFAULT_DRAFT_OBJECTIVE,
    }),
    clearCommandError: assign({
      commandErrorMessage: () => null,
    }),
    assignObjectiveRequiredError: assign({
      commandErrorMessage: () => "Run objective is required.",
    }),
    assignStartRunError: assign(({ event }: any) => ({
      commandErrorMessage: event.error instanceof Error ? event.error.message : String(event.error),
    })),
    resetCommandAfterSuccess: assign({
      commandErrorMessage: () => null,
      draftObjective: () => "",
    }),
    assignSnapshotSuccess: assign(({ context, event }: any) => {
      const hydratedState = hydrateSnapshotState(context, event.output);

      return {
        ...toContextProjection(context, hydratedState),
        afterCursor: event.output.activityPage.latestActivityCursor ?? null,
        refreshPending: context.refreshPending,
      };
    }),
    assignSnapshotError: assign(({ context, event }: any) => ({
      errorMessage: toRunStreamErrorMessage(context.sessionId, event.error),
      isHydrating: false,
      streamStatus: "error",
    })),
    assignStreamFailure: assign({
      errorMessage: ({ event }) => (event.type === "stream.failed" ? event.message : null),
      isHydrating: () => false,
      streamStatus: () => "error",
    }),
    assignStreamMessage: assign(({ context, event }) => {
      if (event.type !== "stream.message") {
        return {};
      }

      const reduced = reduceMessage(context, event.message);
      return {
        ...toContextProjection(context, reduced.next),
      };
    }),
    assignStreamMessageDuringRefresh: assign(({ context, event }) => {
      if (event.type !== "stream.message") {
        return {};
      }

      const reduced = reduceMessage(context, event.message);
      return {
        ...toContextProjection(context, reduced.next),
        refreshPending: context.refreshPending || reduced.needsRefresh,
      };
    }),
    clearRefreshPending: assign({
      refreshPending: () => false,
    }),
    prepareRetry: assign(({ context }) => ({
      isHydrating: true,
      refreshPending: false,
      streamStatus: context.streamStatus,
    })),
    notifyMissingSessionIfNeeded: ({ context, event }: any) => {
      if (isMissingSessionError(event.error, context.sessionId)) {
        context.onSessionInvalid?.(context.sessionId);
      }
    },
    notifyRunStarted: ({ context }) => {
      context.onRunStarted?.();
    },
  },
}).createMachine({
  id: "sessionRuns",
  type: "parallel",
  context: ({ input }) => createInitialSessionRunsContext(input),
  states: {
    command: {
      initial: "idle",
      states: {
        idle: {
          on: {
            draftChanged: {
              actions: "updateDraftObjective",
            },
            startRequested: [
              {
                guard: "hasRunObjective",
                target: "starting",
                actions: "clearCommandError",
              },
              {
                actions: "assignObjectiveRequiredError",
              },
            ],
          },
        },
        starting: {
          invoke: {
            src: "startRunRequest",
            input: ({ context }) => ({
              deps: context.deps,
              objective: context.draftObjective.trim(),
              sessionId: context.sessionId,
            }),
            onDone: {
              target: "idle",
              actions: ["resetCommandAfterSuccess", "notifyRunStarted"],
            },
            onError: {
              target: "idle",
              actions: "assignStartRunError",
            },
          },
          on: {
            draftChanged: {
              actions: "updateDraftObjective",
            },
          },
        },
      },
    },
    stream: {
      initial: "hydrating",
      states: {
        hydrating: {
          invoke: {
            src: "loadSnapshot",
            input: ({ context }) => ({
              deps: context.deps,
              sessionId: context.sessionId,
            }),
            onDone: {
              target: "streaming.idle",
              actions: "assignSnapshotSuccess",
            },
            onError: {
              target: "failed",
              actions: ["assignSnapshotError", "notifyMissingSessionIfNeeded"],
            },
          },
        },
        streaming: {
          invoke: {
            src: "streamSubscription",
            input: ({ context }) => ({
              afterCursor: context.afterCursor,
              deps: context.deps,
              sessionId: context.sessionId,
            }),
          },
          on: {
            "stream.failed": {
              target: "failed",
              actions: "assignStreamFailure",
            },
          },
          initial: "idle",
          states: {
            idle: {
              on: {
                "stream.message": [
                  {
                    guard: "streamMessageNeedsRefresh",
                    target: "refreshing",
                    actions: "assignStreamMessage",
                  },
                  {
                    actions: "assignStreamMessage",
                  },
                ],
              },
            },
            refreshing: {
              invoke: {
                src: "loadSnapshot",
                input: ({ context }) => ({
                  deps: context.deps,
                  sessionId: context.sessionId,
                }),
                onDone: {
                  target: "settling",
                  actions: "assignSnapshotSuccess",
                },
                onError: {
                  target: "#sessionRuns.stream.failed",
                  actions: ["assignSnapshotError", "notifyMissingSessionIfNeeded"],
                },
              },
              on: {
                "stream.message": {
                  actions: "assignStreamMessageDuringRefresh",
                },
              },
            },
            settling: {
              always: [
                {
                  guard: "refreshQueued",
                  target: "refreshing",
                  actions: "clearRefreshPending",
                },
                {
                  target: "idle",
                },
              ],
              on: {
                "stream.message": [
                  {
                    guard: "streamMessageNeedsRefresh",
                    target: "refreshing",
                    actions: "assignStreamMessage",
                  },
                  {
                    actions: "assignStreamMessage",
                  },
                ],
              },
            },
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
      },
    },
  },
});

export function useSessionRunsModel(
  sessionId: SessionId,
  options: UseSessionRunsModelOptions = {},
) {
  const qc = useQueryClient();
  const runsQuery = useSessionRunsQuery(sessionId);
  const activityQuery = useSessionRunActivityQuery(sessionId, RECENT_RUN_ACTIVITY_LIMIT);
  const machineDeps: SessionRunsMachineDeps = options.deps ?? {
    hydrateSnapshot() {},
    async loadSnapshot(targetSessionId) {
      const [runs, activityPage] = await Promise.all([
        qc.fetchQuery({
          queryKey: queryKeys.sessionRuns(targetSessionId),
          queryFn: () => listRuns(targetSessionId),
        }),
        qc.fetchQuery({
          queryKey: queryKeys.sessionActivity(targetSessionId, RECENT_RUN_ACTIVITY_QUERY),
          queryFn: () => getActivityPage(targetSessionId, RECENT_RUN_ACTIVITY_QUERY),
        }),
      ]);
      return {
        activityPage,
        runs,
      };
    },
    subscribeRunStream,
    async startRun(targetSessionId, objective) {
      await startRun(targetSessionId, { objective });
    },
  };
  const [snapshot, send] = useMachine(sessionRunsMachine, {
    input: {
      deps: machineDeps,
      onRunStarted: options.onRunStarted,
      onSessionInvalid: options.onSessionInvalid,
      sessionId,
    },
  });
  const { context } = snapshot;

  return {
    commandErrorMessage: context.commandErrorMessage,
    draftObjective: context.draftObjective,
    errorMessage: context.errorMessage,
    isHydrating: context.isHydrating,
    isStarting: snapshot.matches({
      command: "starting",
    }),
    recentEvents: hydrateRunActivity(activityQuery.data ?? []),
    runs: runsQuery.data ?? [],
    setDraftObjective(value: string) {
      send({
        type: "draftChanged",
        value,
      });
    },
    startRun() {
      send({
        type: "startRequested",
      });
    },
    streamStatus: context.streamStatus,
  };
}

export function createSessionRunsActor(input: SessionRunsMachineInput) {
  return createActor(sessionRunsMachine, { input });
}

export type { RunActivityItem };
