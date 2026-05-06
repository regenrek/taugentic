import type { ActivityPageItem, RunStreamMessage } from "@taugentic/desktop-shared";

export const RECENT_RUN_ACTIVITY_LIMIT = 12;

type ActivityRunEvent = Extract<ActivityPageItem["event"], { run: unknown }>;

export type RunActivityItem = ActivityPageItem & { event: ActivityRunEvent };

export interface SessionRunState {
  errorMessage: string | null;
  isHydrating: boolean;
  streamStatus: "connecting" | "live" | "error";
}

export function createInitialSessionRunState(): SessionRunState {
  return {
    errorMessage: null,
    isHydrating: true,
    streamStatus: "connecting",
  };
}

export function reduceRunStreamMessage(
  state: SessionRunState,
  message: RunStreamMessage,
): { needsRefresh: boolean; state: SessionRunState } {
  if ("status" in message) {
    switch (message.status) {
      case "historyGap":
        return {
          needsRefresh: true,
          state: {
            ...state,
            errorMessage: null,
            isHydrating: true,
            streamStatus: "live",
          },
        };
      case "terminalError":
        return {
          needsRefresh: false,
          state: {
            ...state,
            errorMessage: "run stream entered a terminal error state",
            isHydrating: false,
            streamStatus: "error",
          },
        };
      case "ready":
        return {
          needsRefresh: false,
          state: {
            ...state,
            errorMessage: null,
            streamStatus: "live",
          },
        };
    }
  }

  return {
    needsRefresh: true,
    state: {
      ...state,
      streamStatus: "live",
    },
  };
}

export function toRunStreamErrorMessage(sessionId: string, error: unknown): string {
  const detail = error instanceof Error ? error.message : String(error);
  return `run stream failed for ${sessionId}: ${detail}`;
}

export function hydrateRunActivity(items: ActivityPageItem[]): RunActivityItem[] {
  const recentEvents: RunActivityItem[] = [];
  const seenKeys = new Set<string>();

  for (const item of items) {
    const next = toHydratedRunActivityItem(item);
    if (!next) {
      continue;
    }

    const stableKey = toRunActivityStableKey(next);
    if (seenKeys.has(stableKey)) {
      continue;
    }

    recentEvents.push(next);
    seenKeys.add(stableKey);
    if (recentEvents.length >= RECENT_RUN_ACTIVITY_LIMIT) {
      break;
    }
  }

  return recentEvents;
}

function toHydratedRunActivityItem(item: ActivityPageItem): RunActivityItem | null {
  if (!("run" in item.event)) {
    return null;
  }

  return {
    ...item,
    event: {
      run: item.event.run,
    },
  };
}

function toRunActivityStableKey(item: RunActivityItem): string {
  return item.cursor.sequence.toString();
}
