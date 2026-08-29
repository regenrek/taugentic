import { queryOptions } from "@tanstack/react-query"

import type {
  ActivityPageQuery, ApprovalSnapshotResult, GetRunQuery, GetRunTimelineQuery,
  ListNativeRunsRequest, ListNativeRunsResult, PublicActivityPageResult, RunDetail,
  RunId, RunTimeline, SessionId, SubscribeRunEventsRequest, SubscribeRunEventsResult,
} from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"
import { decodeProtocolJson } from "./protocol-json.js"

export const runActivityQueryRoot = ["daemon", "run-activity"] as const

export function nativeRunsQuery(runtime: DesktopRuntime, sessionId: SessionId, request: ListNativeRunsRequest = { limit: 100 }) {
  return queryOptions({
    queryKey: [...runActivityQueryRoot, sessionId, "runs", request] as const,
    queryFn: async (): Promise<ListNativeRunsResult> => decodeProtocolJson(
      await runtime.bridge.listNativeRuns(sessionId, JSON.stringify(request)),
    ),
  })
}

export function runDetailQuery(runtime: DesktopRuntime, sessionId: SessionId, runId: RunId) {
  const query: GetRunQuery = { runId }
  return queryOptions({
    queryKey: [...runActivityQueryRoot, sessionId, "detail", runId] as const,
    queryFn: async (): Promise<RunDetail | undefined> => decodeProtocolJson(
      await runtime.bridge.getRun(sessionId, JSON.stringify(query)),
    ),
  })
}

export function runTimelineQuery(runtime: DesktopRuntime, sessionId: SessionId, rootRunId: RunId) {
  const query: GetRunTimelineQuery = { sessionId, rootRunId, limit: 200 }
  return queryOptions({
    queryKey: [...runActivityQueryRoot, sessionId, "timeline", rootRunId] as const,
    queryFn: async (): Promise<RunTimeline> => decodeProtocolJson(
      await runtime.bridge.runTimeline(sessionId, JSON.stringify(query)),
    ),
  })
}

export function activityPageQuery(runtime: DesktopRuntime, sessionId: SessionId, query: ActivityPageQuery = { limit: 100 }) {
  return queryOptions({
    queryKey: [...runActivityQueryRoot, sessionId, "activity", query] as const,
    queryFn: (): Promise<PublicActivityPageResult> => getActivityPage(runtime, sessionId, query),
  })
}

export async function getActivityPage(runtime: DesktopRuntime, sessionId: SessionId, query: ActivityPageQuery): Promise<PublicActivityPageResult> {
  return decodeProtocolJson(await runtime.bridge.activityPage(sessionId, JSON.stringify(query)))
}

export function runReplayQuery(runtime: DesktopRuntime, sessionId: SessionId, runId: RunId) {
  const query: SubscribeRunEventsRequest = { sessionId, runId }
  return queryOptions({
    queryKey: [...runActivityQueryRoot, sessionId, "replay", runId] as const,
    queryFn: (): Promise<SubscribeRunEventsResult> => replayRunEvents(runtime, sessionId, query),
  })
}

export function approvalsQuery(runtime: DesktopRuntime, sessionId: SessionId, runId?: RunId) {
  return queryOptions({
    queryKey: [...runActivityQueryRoot, sessionId, "approvals", runId ?? "all"] as const,
    queryFn: async (): Promise<ApprovalSnapshotResult> => decodeProtocolJson(
      await runtime.bridge.listApprovals(JSON.stringify(runId ? { runId } : {})),
    ),
  })
}

export async function replayRunEvents(runtime: DesktopRuntime, sessionId: SessionId, query: SubscribeRunEventsRequest): Promise<SubscribeRunEventsResult> {
  return decodeProtocolJson(await runtime.bridge.replayRunEvents(sessionId, JSON.stringify(query)))
}
