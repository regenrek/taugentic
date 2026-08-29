import { queryOptions } from "@tanstack/react-query"

import type { WorkItemListQuery, WorkItemListResult } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"

export const workItemsQueryKey = ["daemon", "work-items"] as const

/** The only desktop cache for the daemon-owned Work Inbox projection. */
export function workItemsQuery(runtime: DesktopRuntime, query: WorkItemListQuery = {}) {
  return queryOptions({
    queryKey: workItemsQueryKey,
    queryFn: (): Promise<WorkItemListResult> => runtime.listWorkItems(query),
  })
}
