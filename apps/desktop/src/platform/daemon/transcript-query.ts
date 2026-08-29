import { infiniteQueryOptions, type InfiniteData } from "@tanstack/react-query"

import type { ActivityCursor, AgentTurnRow, AgentTurnsPageResult, SessionId } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"
import { decodeProtocolJson } from "./protocol-json.js"
import { desktopQueryClient } from "./query-client.js"

const TRANSCRIPT_PAGE_SIZE = 100
export const transcriptQueryRoot = ["daemon", "transcript"] as const

export function transcriptQueryKey(sessionId: SessionId) {
  return [...transcriptQueryRoot, sessionId] as const
}

/** Pages the daemon/store-owned transcript; no durable message state is rebuilt in TypeScript. */
export function transcriptQuery(runtime: DesktopRuntime, sessionId: SessionId) {
  return infiniteQueryOptions({
    queryKey: transcriptQueryKey(sessionId),
    initialPageParam: undefined as ActivityCursor | undefined,
    queryFn: async ({ pageParam }): Promise<AgentTurnsPageResult> => decodeProtocolJson(
      await runtime.bridge.agentTurnsPage(
        sessionId,
        JSON.stringify({ limit: TRANSCRIPT_PAGE_SIZE, before: pageParam }),
      ),
    ),
    getNextPageParam: (page) => page.nextBefore ?? undefined,
  })
}

export function transcriptRows(pages: readonly AgentTurnsPageResult[] | undefined): AgentTurnRow[] {
  return (pages ?? [])
    .flatMap((page) => page.items ?? [])
    .sort((left, right) => BigInt(left.cursor.sequence) < BigInt(right.cursor.sequence) ? -1 : BigInt(left.cursor.sequence) > BigInt(right.cursor.sequence) ? 1 : 0)
}

export function transcriptHasCommittedAssistant(
  data: InfiniteData<AgentTurnsPageResult> | undefined,
  runId: string,
): boolean {
  return transcriptRows(data?.pages).some((row) => row.kind === "assistant" && row.runId === runId)
}

export function invalidateTranscript(sessionId: SessionId): Promise<void> {
  return desktopQueryClient.invalidateQueries({ queryKey: transcriptQueryKey(sessionId) })
}
