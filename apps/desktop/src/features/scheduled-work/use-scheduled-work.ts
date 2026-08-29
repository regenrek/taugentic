import { useMutation, useQuery } from "@tanstack/react-query"
import { useState } from "react"

import type { AgentRuntimeSelection, ScheduledWorkOccurrence, ScheduledWorkOccurrenceId, SessionId } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { desktopQueryClient } from "../../platform/daemon/query-client.js"
import { scheduledWorkQuery, scheduledWorkQueryKey, type ScheduledWorkRuntime } from "../../platform/daemon/scheduled-work-query.js"

type ScheduledWorkCommands = Pick<DesktopRuntime, "createScheduledWork" | "cancelScheduledWork">

const NO_SESSION = "session-not-selected" as SessionId

function errorMessage(error: unknown): string {
  return error instanceof Error && error.message ? error.message : "Scheduled work could not be updated."
}

function validDueAtMs(value: string): boolean {
  return /^\d+$/.test(value) && Number.isSafeInteger(Number(value))
}

export type ScheduledWorkState = {
  occurrences: readonly ScheduledWorkOccurrence[]
  objective: string
  dueAtMs: string
  loading: boolean
  busy: boolean
  canCreate: boolean
  error?: string
  mutationError?: string
  setObjective(value: string): void
  setDueAtMs(value: string): void
  create(): void
  cancel(occurrenceId: ScheduledWorkOccurrenceId): void
}

/** React Query owns Scheduled Work projection freshness; the draft is transient UI input only. */
export function useScheduledWork(input: {
  runtime: ScheduledWorkRuntime & ScheduledWorkCommands
  enabled: boolean
  sessionId?: SessionId
  selection?: AgentRuntimeSelection
}): ScheduledWorkState {
  const [objective, setObjective] = useState("")
  const [dueAtMs, setDueAtMs] = useState("")
  const enabled = input.enabled && Boolean(input.sessionId)
  const sessionId = input.sessionId ?? NO_SESSION
  const query = useQuery({ ...scheduledWorkQuery(input.runtime, sessionId), enabled })
  const createMutation = useMutation({
    mutationFn: (request: { objective: string; selection: AgentRuntimeSelection; dueAtMs: string }) => input.runtime.createScheduledWork(request),
    onSuccess: () => void desktopQueryClient.invalidateQueries({ queryKey: scheduledWorkQueryKey(sessionId) }),
  })
  const cancelMutation = useMutation({
    mutationFn: (occurrenceId: ScheduledWorkOccurrenceId) => input.runtime.cancelScheduledWork({ occurrenceId }),
    onSuccess: () => void desktopQueryClient.invalidateQueries({ queryKey: scheduledWorkQueryKey(sessionId) }),
  })
  const busy = createMutation.isPending || cancelMutation.isPending
  const canCreate = enabled && !busy && Boolean(input.selection) && Boolean(objective.trim()) && validDueAtMs(dueAtMs)
  const mutationError = createMutation.error ?? cancelMutation.error

  return {
    occurrences: enabled ? query.data?.occurrences ?? [] : [],
    objective,
    dueAtMs,
    loading: enabled && query.isLoading,
    busy,
    canCreate,
    error: enabled && query.isError ? "Scheduled work could not be loaded." : undefined,
    mutationError: mutationError ? errorMessage(mutationError) : undefined,
    setObjective,
    setDueAtMs,
    create: () => {
      if (!canCreate || !input.selection) return
      createMutation.mutate({ objective: objective.trim(), selection: input.selection, dueAtMs })
    },
    cancel: (occurrenceId) => {
      if (enabled && !busy) cancelMutation.mutate(occurrenceId)
    },
  }
}
