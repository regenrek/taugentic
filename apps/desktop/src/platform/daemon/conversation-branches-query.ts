import { queryOptions, type QueryClient } from "@tanstack/react-query"

import type { RunLineageGraphResult, SessionId } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"

export const conversationBranchesQueryRoot = ["daemon", "conversation-branches"] as const

export function conversationBranchesQueryKey(sessionId: SessionId) {
  return [...conversationBranchesQueryRoot, sessionId] as const
}

/**
 * Lifecycle recovery owns one cache invalidation for the selected session's
 * daemon-projected fork rows. Active observers refetch that single projection.
 */
export async function invalidateConversationBranchesForLifecycleRecovery(
  queryClient: QueryClient,
  sessionId: SessionId,
): Promise<void> {
  await queryClient.invalidateQueries({ queryKey: conversationBranchesQueryKey(sessionId) })
}

/**
 * One bounded daemon projection owns lineage traversal; this query never pages
 * or recursively fetches graph facts.
 */
export function conversationBranchesQuery(runtime: DesktopRuntime, sessionId: SessionId) {
  return queryOptions({
    queryKey: conversationBranchesQueryKey(sessionId),
    queryFn: (): Promise<RunLineageGraphResult> => runtime.runLineageGraph(sessionId),
  })
}

/** The generated relationship discriminant is the sole branch-kind owner. */
export function conversationBranchRows(graph: RunLineageGraphResult | undefined) { return graph?.nodes ?? [] }
