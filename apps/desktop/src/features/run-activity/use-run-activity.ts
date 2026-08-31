import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import type { AgentRuntimeSelection, ApprovalDecision, ApprovalId, ApprovalRequest, ArtifactId, RunId, SessionId, SwitchRouteAndResumeRequest } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { getActivityPage, getNativeRunsPage, runActivityQueryRoot, runDetailQuery, runReplayQuery, runTimelineQuery } from "../../platform/daemon/run-activity-query.js"

const NO_SESSION = "session-not-selected" as SessionId

/** The one desktop-side owner for the explicit replacement-route intent. */
export async function requestSwitchRouteAndResume(
  runtime: Pick<DesktopRuntime, "switchRouteAndResume">,
  input: {
    sessionId?: SessionId
    parentRunId?: RunId
    exhausted: boolean
    replacementSelection?: AgentRuntimeSelection
  },
): Promise<boolean> {
  if (!input.sessionId || !input.parentRunId || !input.exhausted || !input.replacementSelection) {
    return false
  }
  const request: SwitchRouteAndResumeRequest = {
    sessionId: input.sessionId,
    parentRunId: input.parentRunId,
    selection: input.replacementSelection,
  }
  await runtime.switchRouteAndResume(request)
  return true
}

export function useRunActivity(input: { runtime: DesktopRuntime; sessionId?: SessionId; replacementSelection?: AgentRuntimeSelection; enabled: boolean; approvals: readonly ApprovalRequest[]; decideApproval(approvalId: ApprovalId, decision: ApprovalDecision): Promise<void>; openArtifact(artifactId: ArtifactId): void }) {
  const sessionId = input.sessionId ?? NO_SESSION
  const queryClient = useQueryClient()
  const [selectedRunId, setSelectedRunId] = useState<RunId>()
  const enabled = input.enabled && Boolean(input.sessionId)
  const runs = useInfiniteQuery({
    queryKey: [...runActivityQueryRoot, sessionId, "runs"] as const,
    initialPageParam: undefined as string | undefined,
    retry: false,
    queryFn: ({ pageParam }) => getNativeRunsPage(input.runtime, sessionId, pageParam),
    getNextPageParam: (page) => page.nextCursor ?? undefined,
    enabled,
  })
  const runHistory = runs.data?.pages.flatMap((page) => page.runs) ?? []
  const firstRunId = runs.data?.pages[0]?.runs[0]?.id
  useEffect(() => {
    if (!selectedRunId && firstRunId) setSelectedRunId(firstRunId)
  }, [firstRunId, selectedRunId])
  useEffect(() => setSelectedRunId(undefined), [sessionId])
  const detail = useQuery({ ...runDetailQuery(input.runtime, sessionId, selectedRunId ?? ("run-not-selected" as RunId)), enabled: enabled && Boolean(selectedRunId) })
  const timeline = useQuery({ ...runTimelineQuery(input.runtime, sessionId, selectedRunId ?? ("run-not-selected" as RunId)), enabled: enabled && Boolean(selectedRunId) })
  const activity = useInfiniteQuery({
    queryKey: [...runActivityQueryRoot, sessionId, "activity"] as const,
    initialPageParam: undefined as string | undefined,
    retry: false,
    queryFn: ({ pageParam }) => getActivityPage(input.runtime, sessionId, {
      limit: 100,
      ...(pageParam === undefined ? {} : { before: { sequence: pageParam } }),
    }),
    getNextPageParam: (page) => page.nextBefore?.sequence ?? undefined,
    enabled,
  })
  const replay = useQuery({ ...runReplayQuery(input.runtime, sessionId, selectedRunId ?? ("run-not-selected" as RunId)), enabled: enabled && Boolean(selectedRunId) })
  const refresh = () => { void queryClient.invalidateQueries({ queryKey: [...runActivityQueryRoot, sessionId] }) }
  const switchEligible = Boolean(input.sessionId && selectedRunId && detail.data?.authProfileExhaustion && input.replacementSelection)
  return {
    runs: runHistory, selectedRunId, selectRun: setSelectedRunId,
    detail: detail.data, timeline: timeline.data, replay: replay.data?.events ?? [],
    activity: activity.data?.pages.flatMap((page) => page.items ?? []) ?? [],
    approvals: input.approvals.filter((approval) => approval.runId === selectedRunId),
    hasOlderActivity: Boolean(activity.hasNextPage),
    loadingOlderActivity: activity.isFetchingNextPage,
    loadOlderActivity: () => { void activity.fetchNextPage({ cancelRefetch: true }) },
    hasOlderRuns: Boolean(runs.hasNextPage),
    loadingOlderRuns: runs.isFetchingNextPage,
    loadOlderRuns: () => { void runs.fetchNextPage({ cancelRefetch: true }) },
    loading: runs.isLoading || detail.isLoading || timeline.isLoading,
    error: runs.isError || detail.isError || timeline.isError || activity.isError || replay.isError ? "Run activity could not be loaded." : undefined,
    refresh,
    decide: input.decideApproval,
    cancel: async (runId: RunId) => { await input.runtime.bridge.cancelRun(runId); refresh() },
    switchRouteAndResume: async () => {
      const started = await requestSwitchRouteAndResume(input.runtime, {
        sessionId: input.sessionId,
        parentRunId: selectedRunId,
        exhausted: Boolean(detail.data?.authProfileExhaustion),
        replacementSelection: input.replacementSelection,
      })
      if (started) refresh()
    },
    switchEligible,
    openArtifact: input.openArtifact,
  }
}
