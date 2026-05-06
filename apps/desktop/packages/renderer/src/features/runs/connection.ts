import type {
  ActivityPageQuery,
  ActivityPageResult,
  RunStreamMessage,
  RunSummary,
  SessionId,
} from "@taugentic/desktop-shared";
import type { StreamUnsubscribe } from "../../lib/ipc/stream";

import { RECENT_RUN_ACTIVITY_LIMIT } from "./state";

export interface SessionRunConnectionDeps {
  hydrateSnapshot(snapshot: RunSnapshotRefreshResult): void;
  loadSnapshot(sessionId: SessionId): Promise<RunSnapshotRefreshResult>;
  subscribeRunStream(
    sessionId: SessionId,
    afterCursor: ActivityPageResult["latestActivityCursor"],
    onMessage: (message: RunStreamMessage) => void,
    onError?: (error: Error) => void,
  ): Promise<StreamUnsubscribe>;
}

export interface RunSnapshotRefreshResult {
  activityPage: ActivityPageResult;
  runs: RunSummary[];
}

export const RECENT_RUN_ACTIVITY_QUERY = {
  kinds: ["run"],
  limit: RECENT_RUN_ACTIVITY_LIMIT,
} as const satisfies ActivityPageQuery;
