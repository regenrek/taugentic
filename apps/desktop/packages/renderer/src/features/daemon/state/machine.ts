import { assign, fromPromise, setup } from "xstate";

import {
  createInitialDaemonControlState,
  type DaemonControlDeps,
  type DaemonControlState,
  type PendingDaemonAction,
} from "../types";

type DaemonControlRequestAction = Exclude<PendingDaemonAction, null>;

interface DaemonControlMachineContext extends DaemonControlState {
  completion: (() => void) | null;
  deps: DaemonControlDeps;
}

type DaemonControlMachineEvent = {
  type: "daemonActionRequested";
  action: DaemonControlRequestAction;
  completion?: () => void;
};

function formatDaemonControlError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function runDaemonControlAction(action: DaemonControlRequestAction, deps: DaemonControlDeps) {
  switch (action) {
    case "disable-background":
      return deps.disableBackground();
    case "enable-background":
      return deps.enableBackground();
    case "reconcile":
      return deps.reconcile();
    case "refresh":
      return deps.refresh();
    case "start":
      return deps.start();
    case "stop":
      return deps.stop();
  }
}

export const daemonControlMachine = setup({
  types: {
    context: {} as DaemonControlMachineContext,
    events: {} as DaemonControlMachineEvent,
    input: {} as {
      deps: DaemonControlDeps;
    },
  },
  actors: {
    performDaemonAction: fromPromise(
      async ({
        input,
      }: {
        input: {
          action: DaemonControlRequestAction;
          deps: DaemonControlDeps;
        };
      }) => runDaemonControlAction(input.action, input.deps),
    ),
  },
  actions: {
    assignRequestedAction: assign(({ context, event }) => {
      if (event.type !== "daemonActionRequested") {
        return {};
      }
      return {
        ...context,
        completion: event.completion ?? null,
        errorMessage: null,
        pendingAction: event.action,
      };
    }),
    resolvePendingCompletion: ({ context }) => {
      context.completion?.();
    },
  },
}).createMachine({
  id: "daemonControl",
  context: ({ input }) => ({
    ...createInitialDaemonControlState(),
    completion: null,
    deps: input.deps,
  }),
  initial: "idle",
  states: {
    idle: {
      on: {
        daemonActionRequested: {
          actions: "assignRequestedAction",
          target: "pending",
        },
      },
    },
    pending: {
      invoke: {
        id: "performDaemonAction",
        src: "performDaemonAction",
        input: ({ context }) => {
          if (context.pendingAction === null) {
            throw new Error("Daemon action missing while pending.");
          }
          return {
            action: context.pendingAction,
            deps: context.deps,
          };
        },
        onDone: {
          actions: [
            "resolvePendingCompletion",
            assign(({ event }) => ({
              completion: null,
              errorMessage: null,
              pendingAction: null,
              state: event.output,
            })),
          ],
          target: "idle",
        },
        onError: {
          actions: [
            "resolvePendingCompletion",
            assign(({ event }) => ({
              completion: null,
              errorMessage: formatDaemonControlError(event.error),
              pendingAction: null,
            })),
          ],
          target: "idle",
        },
      },
      on: {
        daemonActionRequested: {
          actions: ["resolvePendingCompletion", "assignRequestedAction"],
          reenter: true,
          target: "pending",
        },
      },
    },
  },
});

export function requestDaemonControlAction(
  actorRef: {
    send: (event: DaemonControlMachineEvent) => void;
  },
  action: DaemonControlRequestAction,
): Promise<void> {
  return new Promise((resolve) => {
    actorRef.send({
      action,
      completion: resolve,
      type: "daemonActionRequested",
    });
  });
}
