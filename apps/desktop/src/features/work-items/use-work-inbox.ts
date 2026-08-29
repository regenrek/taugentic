import { useMutation, useQuery } from "@tanstack/react-query"

import type { WorkItem, WorkItemKey } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { desktopQueryClient } from "../../platform/daemon/query-client.js"
import { workItemsQuery, workItemsQueryKey } from "../../platform/daemon/work-items-query.js"

function errorMessage(error: unknown): string {
  return error instanceof Error && error.message ? error.message : "The Work Inbox could not be updated."
}

export type WorkInboxState = {
  items: readonly WorkItem[]
  sync?: import("@taugentic/desktop-protocol").WorkSourceSyncStatus
  loading: boolean
  busy: boolean
  actionsEnabled: boolean
  error?: string
  mutationError?: string
  refresh(): void
  dismiss(key: WorkItemKey): void
  trigger(item: WorkItem): void
}

/** React Query projects daemon truth; this hook owns no WorkItem lifecycle. */
export function useWorkInbox(input: {
  runtime: DesktopRuntime
  enabled: boolean
  canTrigger: boolean
  trigger(key: WorkItemKey): Promise<void>
}): WorkInboxState {
  const query = useQuery({ ...workItemsQuery(input.runtime), enabled: input.enabled })
  const refreshMutation = useMutation({
    mutationFn: () => input.runtime.refreshWorkItems(),
    onSuccess: (projection) => {
      desktopQueryClient.setQueryData(workItemsQueryKey, projection)
    },
  })
  const dismissMutation = useMutation({
    mutationFn: (key: WorkItemKey) => input.runtime.dismissWorkItem({ key }),
    onSuccess: () => {
      void desktopQueryClient.invalidateQueries({ queryKey: workItemsQueryKey })
    },
  })
  const triggerMutation = useMutation({
    mutationFn: (key: WorkItemKey) => input.trigger(key),
    onSuccess: () => {
      void desktopQueryClient.invalidateQueries({ queryKey: workItemsQueryKey })
    },
  })
  const mutation = refreshMutation.isPending || dismissMutation.isPending || triggerMutation.isPending
  const mutationError = refreshMutation.error ?? dismissMutation.error ?? triggerMutation.error
  return {
    items: query.data?.items ?? [],
    sync: query.data?.sync,
    loading: query.isLoading,
    busy: mutation,
    actionsEnabled: input.enabled,
    error: query.isError ? "The Work Inbox could not be loaded." : undefined,
    mutationError: mutationError ? errorMessage(mutationError) : undefined,
    refresh: () => { if (input.enabled && !mutation) refreshMutation.mutate() },
    dismiss: (key) => { if (input.enabled && !mutation) dismissMutation.mutate(key) },
    trigger: (item) => {
      if (!mutation && input.canTrigger && item.status === "available") triggerMutation.mutate(item.key)
    },
  }
}
