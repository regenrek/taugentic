import { useQuery, type UseQueryResult } from "@tanstack/react-query";

import {
  DEFAULT_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT,
  type ConflictWarning,
  type ActivityPageQuery,
  type ActivityPageResult,
  type ApprovalSnapshotResult,
  type ApprovalRequest,
  type ArtifactSnapshotResult,
  type ArtifactSummary,
  type ListNativeRunsRequest,
  type ListNativeRunsResult,
  type PublicActivityPageItem,
  type RunEventDelta,
  type RunDetail,
  type RunId,
  type RunSummary,
  type RunTimeline,
  type SessionId,
  type SessionOverviewResult,
  type SessionSummary,
} from "@taugentic/desktop-shared";

import {
  getActivityPage,
  getRunDetail,
  getSessionOverview,
  listSessions,
  listApprovals,
  listArtifacts,
  listNativeRuns,
  listRuns,
  getRunTimeline,
  replayRunEvents,
} from "@/lib/ipc/api";

import { queryKeys } from "./keys";

/**
 * Default page size for the per-session activity feed. Matches the value the
 * daemon keeps in memory for `getActivityPage`.
 */
const DEFAULT_ACTIVITY_PAGE_LIMIT = 100;
export const DEFAULT_AGENT_TURNS_PAGE_LIMIT = 100;
const DEFAULT_RUN_CONFLICT_WARNING_LIMIT = 100;

/** Canonical polling cadence for all session queries in the workspace shell. */
export const SESSION_QUERY_POLL_INTERVAL_MS = 2000;

export interface SessionQueryView<TData> {
  data: TData | undefined;
  error: unknown;
  isLoading: boolean;
  isFetching: boolean;
  refetch: UseQueryResult<TData>["refetch"];
}

export interface RunConflictWarningItem {
  occurredAtMs: bigint;
  runId: RunId;
  warning: ConflictWarning;
}

function toView<TData>(query: UseQueryResult<TData>): SessionQueryView<TData> {
  return {
    data: query.data,
    error: query.error,
    isLoading: query.isLoading,
    isFetching: query.isFetching,
    refetch: query.refetch,
  };
}

function activityQueryKey(
  sessionId: SessionId | null,
  query: Pick<ActivityPageQuery, "kinds" | "limit">,
) {
  if (sessionId === null) {
    return [
      "session",
      "__none__",
      "activity",
      {
        kinds: query.kinds ? [...query.kinds].sort() : null,
        limit: query.limit,
      },
    ] as const;
  }

  return queryKeys.sessionActivity(sessionId, query);
}

export interface UseSessionOverviewQueryOptions {
  recentActivityLimit?: number;
}

export function useSessionsQuery(): SessionQueryView<SessionSummary[]> {
  const query = useQuery({
    queryKey: queryKeys.sessions,
    queryFn: () => listSessions(),
  });
  return toView(query);
}

export function useSessionOverviewQuery(
  options: UseSessionOverviewQueryOptions = {},
): SessionQueryView<SessionOverviewResult> {
  const recentActivityLimit =
    options.recentActivityLimit ?? DEFAULT_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT;
  const query = useQuery({
    queryKey: queryKeys.sessionOverview(recentActivityLimit),
    queryFn: () => getSessionOverview({ recentActivityLimit }),
    refetchInterval: SESSION_QUERY_POLL_INTERVAL_MS,
  });
  return toView(query);
}

export function useSessionRunsQuery(sessionId: SessionId | null): SessionQueryView<RunSummary[]> {
  const query = useQuery({
    enabled: sessionId !== null,
    queryKey:
      sessionId === null
        ? (["session", "__none__", "runs"] as const)
        : queryKeys.sessionRuns(sessionId),
    queryFn: () => listRuns(sessionId as SessionId),
  });
  return toView(query);
}

export function useSessionNativeRunsQuery(
  sessionId: SessionId | null,
  request: ListNativeRunsRequest,
): SessionQueryView<ListNativeRunsResult> {
  const query = useQuery({
    enabled: sessionId !== null,
    queryKey:
      sessionId === null
        ? (["session", "__none__", "nativeRuns"] as const)
        : queryKeys.sessionNativeRuns(sessionId, request),
    queryFn: () => listNativeRuns(sessionId as SessionId, request),
  });
  return toView(query);
}

export function useRunDetailQuery(
  sessionId: SessionId | null,
  runId: RunId | null,
): SessionQueryView<RunDetail | null> {
  const query = useQuery({
    enabled: sessionId !== null && runId !== null,
    queryKey:
      sessionId === null || runId === null
        ? (["session", "__none__", "runDetail"] as const)
        : queryKeys.sessionRunDetail(sessionId, runId),
    queryFn: () => getRunDetail(sessionId as SessionId, runId as RunId),
    refetchInterval: SESSION_QUERY_POLL_INTERVAL_MS,
  });
  return toView(query);
}

export function useRunEventTimelineQuery(
  sessionId: SessionId | null,
  runId: RunId | null,
  afterSeq: bigint | null = null,
): SessionQueryView<RunEventDelta[]> {
  const query = useQuery({
    enabled: sessionId !== null && runId !== null,
    queryKey:
      sessionId === null || runId === null
        ? (["session", "__none__", "runEvents"] as const)
        : queryKeys.sessionRunEvents(sessionId, runId, afterSeq),
    queryFn: () => replayRunEvents(sessionId as SessionId, runId as RunId, afterSeq),
    refetchInterval: SESSION_QUERY_POLL_INTERVAL_MS,
    select: (result) => result.events,
  });
  return toView(query);
}

export function useRunTimelineQuery(
  sessionId: SessionId | null,
  rootRunId: RunId | null,
): SessionQueryView<RunTimeline> {
  const query = useQuery({
    enabled: sessionId !== null && rootRunId !== null,
    queryKey:
      sessionId === null || rootRunId === null
        ? (["session", "__none__", "runTimeline"] as const)
        : queryKeys.sessionRunTimeline(sessionId, rootRunId),
    queryFn: () => getRunTimeline(sessionId as SessionId, rootRunId as RunId),
    refetchInterval: SESSION_QUERY_POLL_INTERVAL_MS,
  });
  return toView(query);
}

export function useRunConflictWarningsQuery(
  sessionId: SessionId | null,
  runId: RunId | null,
  limit: number = DEFAULT_RUN_CONFLICT_WARNING_LIMIT,
): SessionQueryView<RunConflictWarningItem[]> {
  const activityQuery = {
    kinds: ["conflict"] as const,
    limit,
  } satisfies Pick<ActivityPageQuery, "kinds" | "limit">;
  const query = useQuery({
    enabled: sessionId !== null && runId !== null,
    queryKey:
      sessionId === null || runId === null
        ? (["session", "__none__", "runConflicts"] as const)
        : queryKeys.sessionRunConflicts(sessionId, runId, limit),
    queryFn: () => getActivityPage(sessionId as SessionId, activityQuery),
    refetchInterval: SESSION_QUERY_POLL_INTERVAL_MS,
    select: (page: ActivityPageResult) =>
      (page.items ?? []).flatMap((item) =>
        conflictWarningForRun(item, runId as RunId).map((warning) => ({
          occurredAtMs: item.occurredAtMs,
          runId: warning.runId,
          warning: warning.warning,
        })),
      ),
  });
  return toView(query);
}

export function useSessionActivityQuery(
  sessionId: SessionId | null,
  limit: number = DEFAULT_ACTIVITY_PAGE_LIMIT,
): SessionQueryView<PublicActivityPageItem[]> {
  const activityQuery = { limit } satisfies Pick<ActivityPageQuery, "limit">;
  const query = useQuery({
    enabled: sessionId !== null,
    queryKey: activityQueryKey(sessionId, activityQuery),
    queryFn: () => getActivityPage(sessionId as SessionId, activityQuery),
    select: (page: ActivityPageResult) => page.items ?? [],
  });
  return toView(query);
}

function conflictWarningForRun(
  item: PublicActivityPageItem,
  runId: RunId,
): Array<{ runId: RunId; warning: ConflictWarning }> {
  if (!("conflict" in item.event) || item.event.conflict.phase !== "warning") {
    return [];
  }

  const { warning } = item.event.conflict;
  if (
    item.event.conflict.run_id !== runId &&
    warning.requestingCapsule !== runId &&
    !warning.conflicts.some((conflict) => conflict.holdingCapsule === runId)
  ) {
    return [];
  }

  return [
    {
      runId: item.event.conflict.run_id,
      warning,
    },
  ];
}

export function useSessionRunActivityQuery(
  sessionId: SessionId | null,
  limit: number,
): SessionQueryView<PublicActivityPageItem[]> {
  const activityQuery = {
    kinds: ["run"] as const,
    limit,
  } satisfies Pick<ActivityPageQuery, "kinds" | "limit">;
  const query = useQuery({
    enabled: sessionId !== null,
    queryKey: activityQueryKey(sessionId, activityQuery),
    queryFn: () => getActivityPage(sessionId as SessionId, activityQuery),
    select: (page: ActivityPageResult) => page.items ?? [],
  });
  return toView(query);
}

export function useSessionApprovalsQuery(
  sessionId: SessionId | null,
): SessionQueryView<ApprovalRequest[]> {
  const query = useQuery({
    enabled: sessionId !== null,
    queryKey:
      sessionId === null
        ? (["session", "__none__", "approvals"] as const)
        : queryKeys.sessionApprovals(sessionId),
    queryFn: () => listApprovals(sessionId as SessionId, {}),
    refetchInterval: SESSION_QUERY_POLL_INTERVAL_MS,
    select: (snapshot: ApprovalSnapshotResult) => snapshot.items,
  });
  return toView(query);
}

export function useSessionArtifactsQuery(
  sessionId: SessionId | null,
): SessionQueryView<ArtifactSummary[]> {
  const query = useQuery({
    enabled: sessionId !== null,
    queryKey:
      sessionId === null
        ? (["session", "__none__", "artifacts"] as const)
        : queryKeys.sessionArtifacts(sessionId),
    queryFn: () => listArtifacts(sessionId as SessionId, {}),
    select: (snapshot: ArtifactSnapshotResult) => snapshot.items,
  });
  return toView(query);
}
