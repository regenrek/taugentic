import type {
  ActivityPageQuery,
  AgentTurnsPageQuery,
  ListNativeRunsRequest,
  RunId,
  SessionId,
} from "@taugentic/desktop-shared";

function normalizeActivityQuery(query: Pick<ActivityPageQuery, "kinds" | "limit">) {
  return {
    kinds: query.kinds ? [...query.kinds].sort() : null,
    limit: query.limit,
  } as const;
}

function normalizeAgentTurnsQuery(query: Pick<AgentTurnsPageQuery, "limit">) {
  return {
    limit: query.limit,
  } as const;
}

function normalizeNativeRunsQuery(query: ListNativeRunsRequest) {
  return {
    cursor: query.cursor ?? null,
    filter: query.filter
      ? {
          harness: query.filter.harness ? [...query.filter.harness].sort() : null,
          parentRunId: query.filter.parentRunId ?? null,
          status: query.filter.status ? [...query.filter.status].sort() : null,
        }
      : null,
    limit: query.limit,
  } as const;
}

export const queryKeys = {
  daemon: {
    diagnostics: ["daemon", "diagnostics"] as const,
    status: ["daemon", "status"] as const,
  },
  agentRuntime: {
    snapshot: ["agentRuntime", "snapshot"] as const,
  },
  recipes: ["recipes"] as const,
  workflow: {
    status: ["workflow", "status"] as const,
  },
  workItems: ["workItems"] as const,
  sessions: ["session", "list"] as const,
  sessionOverview: (recentActivityLimit: number) =>
    ["session", "overview", { recentActivityLimit }] as const,
  session: (id: SessionId) => ["session", id] as const,
  sessionRuns: (id: SessionId) => [...queryKeys.session(id), "runs"] as const,
  sessionRunDetail: (id: SessionId, runId: RunId) =>
    [...queryKeys.session(id), "runDetail", runId] as const,
  sessionRunEvents: (id: SessionId, runId: RunId, afterSeq: bigint | null) =>
    [
      ...queryKeys.session(id),
      "runEvents",
      runId,
      { afterSeq: afterSeq?.toString() ?? null },
    ] as const,
  sessionRunTimeline: (id: SessionId, rootRunId: RunId) =>
    [...queryKeys.session(id), "runTimeline", rootRunId] as const,
  sessionRunConflicts: (id: SessionId, runId: RunId, limit: number) =>
    [...queryKeys.session(id), "runConflicts", runId, { limit }] as const,
  sessionNativeRunsRoot: (id: SessionId) => [...queryKeys.session(id), "nativeRuns"] as const,
  sessionNativeRuns: (id: SessionId, query: ListNativeRunsRequest) =>
    [...queryKeys.sessionNativeRunsRoot(id), normalizeNativeRunsQuery(query)] as const,
  sessionActivityRoot: (id: SessionId) => [...queryKeys.session(id), "activity"] as const,
  sessionActivity: (id: SessionId, query: Pick<ActivityPageQuery, "kinds" | "limit">) =>
    [...queryKeys.session(id), "activity", normalizeActivityQuery(query)] as const,
  sessionAgentTurnsRoot: (id: SessionId) => [...queryKeys.session(id), "agentTurns"] as const,
  sessionAgentTurns: (id: SessionId, query: Pick<AgentTurnsPageQuery, "limit">) =>
    [...queryKeys.session(id), "agentTurns", normalizeAgentTurnsQuery(query)] as const,
  sessionApprovals: (id: SessionId) => [...queryKeys.session(id), "approvals"] as const,
  sessionArtifacts: (id: SessionId) => [...queryKeys.session(id), "artifacts"] as const,
};

/** Root key used to invalidate every session overview variant regardless of limit. */
export const sessionOverviewRootKey = ["session", "overview"] as const;
export const sessionListRootKey = queryKeys.sessions;
export const agentRuntimeRootKey = ["agentRuntime"] as const;
