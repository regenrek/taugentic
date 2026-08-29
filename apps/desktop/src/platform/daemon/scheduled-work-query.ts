import { queryOptions } from "@tanstack/react-query"

import type { ListScheduledWorkResult, SessionId } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"

export type ScheduledWorkRuntime = Pick<DesktopRuntime, "listScheduledWork">

export const scheduledWorkQueryRoot = ["daemon", "scheduled-work"] as const

/** Scheduled Work is attached to one daemon session, so its cache must be too. */
export function scheduledWorkQueryKey(sessionId: SessionId) {
  return [...scheduledWorkQueryRoot, sessionId] as const
}

/** The one desktop cache for daemon-owned Scheduled Work occurrences. */
export function scheduledWorkQuery(runtime: ScheduledWorkRuntime, sessionId: SessionId) {
  return queryOptions({
    queryKey: scheduledWorkQueryKey(sessionId),
    queryFn: (): Promise<ListScheduledWorkResult> => runtime.listScheduledWork(),
  })
}
