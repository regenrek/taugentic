import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query"
import { useEffect, useState } from "react"

import type { AgentRuntimeSelection, ApprovalDecision, ApprovalId, ApprovalRequest, ArtifactId, RunId, SessionId, SwitchAccountAndResumeRequest } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { getActivityPage, nativeRunsQuery, runActivityQueryRoot, runDetailQuery, runReplayQuery, runTimelineQuery } from "../../platform/daemon/run-activity-query.js"

const NO_SESSION = "session-not-selected" as SessionId

/** The one desktop-side owner for the explicit account replacement request. */
export async function requestSwitchAccountAndResume(
  runtime: Pick<DesktopRuntime, "switchAccountAndResume">,
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
  const request: SwitchAccountAndResumeRequest = {
    sessionId: input.sessionId,
    parentRunId: input.parentRunId,
    selection: input.replacementSelection,
  }
  await runtime.switchAccountAndResume(request)
  return true
}

export function useRunActivity(input: { runtime: DesktopRuntime; sessionId?: SessionId; replacementSelection?: AgentRuntimeSelection; enabled: boolean; approvals: readonly ApprovalRequest[]; decideApproval(approvalId: ApprovalId, decision: ApprovalDecision): Promise<void>; openArtifact(artifactId: ArtifactId): void }) {
  const sessionId = input.sessionId ?? NO_SESSION
  const queryClient = useQueryClient()
  const [selectedRunId, setSelectedRunId] = useState<RunId>()
  const enabled = input.enabled && Boolean(input.sessionId)
  const runs = useQuery({ ...nativeRunsQuery(input.runtime, sessionId), enabled })
  useEffect(() => {
    if (!selectedRunId && runs.data?.runs[0]) setSelectedRunId(runs.data.runs[0].id)
  }, [runs.data?.runs, selectedRunId])
  useEffect(() => setSelectedRunId(undefined), [sessionId])
  const detail = useQuery({ ...runDetailQuery(input.runtime, sessionId, selectedRunId ?? ("run-not-selected" as RunId)), enabled: enabled && Boolean(selectedRunId) })
  const timeline = useQuery({ ...runTimelineQuery(input.runtime, sessionId, selectedRunId ?? ("run-not-selected" as RunId)), enabled: enabled && Boolean(selectedRunId) })
  const activity = useInfiniteQuery({
    queryKey: [...runActivityQueryRoot, sessionId, "activity"] as const,
    initialPageParam: undefined as string | undefined,
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
    runs: runs.data?.runs ?? [], selectedRunId, selectRun: setSelectedRunId,
    detail: detail.data, timeline: timeline.data, replay: replay.data?.events ?? [],
    activity: activity.data?.pages.flatMap((page) => page.items ?? []) ?? [],
    approvals: input.approvals.filter((approval) => approval.runId === selectedRunId),
    hasOlderActivity: Boolean(activity.hasNextPage),
    loadingOlderActivity: activity.isFetchingNextPage,
    loadOlderActivity: () => { void activity.fetchNextPage({ cancelRefetch: true }) },
    loading: runs.isLoading || detail.isLoading || timeline.isLoading,
    error: runs.isError || detail.isError || timeline.isError || activity.isError || replay.isError ? "Run activity could not be loaded." : undefined,
    refresh,
    decide: input.decideApproval,
    cancel: async (runId: RunId) => { await input.runtime.bridge.cancelRun(runId); refresh() },
    switchAccountAndResume: async () => {
      const started = await requestSwitchAccountAndResume(input.runtime, {
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
