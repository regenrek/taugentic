import { useMutation, useQuery } from "@tanstack/react-query"
import { useCallback, useEffect, useRef, useState } from "react"

import type { SessionId, ThreadWorkspaceMutation, ThreadWorkspaceResult } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { threadWorkspaceQuery, threadWorkspaceQueryKey, updateThreadWorkspace } from "../../platform/daemon/thread-workspace-query.js"
import { desktopQueryClient } from "../../platform/daemon/query-client.js"

const NO_SESSION = "session-not-selected" as SessionId

type DraftField = "goal" | "plan" | "recap" | "notes"
type Drafts = Record<DraftField, string>
type DirtyDrafts = Record<DraftField, boolean>

const emptyDrafts: Drafts = { goal: "", plan: "", recap: "", notes: "" }
const cleanDrafts: DirtyDrafts = { goal: false, plan: false, recap: false, notes: false }

function projectionDrafts(projection: ThreadWorkspaceResult | undefined): Drafts {
  return projection
    ? { goal: projection.goal, plan: projection.plan, recap: projection.recap, notes: projection.notes }
    : emptyDrafts
}

function errorMessage(error: unknown): string {
  return error instanceof Error && error.message ? error.message : "Thread workspace could not be updated."
}

export type ThreadWorkspacePanelState = {
  sessionId?: SessionId
  projection?: ThreadWorkspaceResult
  loading: boolean
  error?: string
  drafts: Drafts
  dirty: DirtyDrafts
  busy: boolean
  mutationError?: string
  setDraft(field: DraftField, value: string): void
  save(field: DraftField): void
  addPin(runId: string, cursor: string): void
  removePin(cursor: string): void
  refresh(): void
}

/** Presentation drafts are ephemeral; result data and mutations remain daemon-projected. */
export function useThreadWorkspace(input: { runtime: DesktopRuntime; sessionId?: SessionId; enabled: boolean }): ThreadWorkspacePanelState {
  const sessionId = input.sessionId ?? NO_SESSION
  const [drafts, setDrafts] = useState<Drafts>(emptyDrafts)
  const [dirty, setDirty] = useState<DirtyDrafts>(cleanDrafts)
  const [draftSessionId, setDraftSessionId] = useState<SessionId>()
  const [mutationError, setMutationError] = useState<string>()
  const draftsRef = useRef(drafts)
  useEffect(() => { draftsRef.current = drafts }, [drafts])
  const query = useQuery({
    ...threadWorkspaceQuery(input.runtime, sessionId),
    enabled: input.enabled && Boolean(input.sessionId),
  })
  const mutation = useMutation({
    mutationFn: (next: ThreadWorkspaceMutation) => updateThreadWorkspace(input.runtime, next),
    onSuccess: (projection, submitted) => {
      desktopQueryClient.setQueryData(threadWorkspaceQueryKey(sessionId), projection)
      setMutationError(undefined)
      if (submitted.kind === "goalSet" || submitted.kind === "planSet" || submitted.kind === "recapSet" || submitted.kind === "notesSet") {
        const field = submitted.kind === "goalSet" ? "goal"
          : submitted.kind === "planSet" ? "plan"
            : submitted.kind === "recapSet" ? "recap"
              : "notes"
        if (draftsRef.current[field] === submitted.value) {
          setDirty((current) => ({ ...current, [field]: false }))
        }
      }
    },
    onError: (error) => setMutationError(errorMessage(error)),
  })

  useEffect(() => {
    const projection = query.data
    if (draftSessionId !== sessionId) {
      const nextDrafts = projectionDrafts(projection)
      draftsRef.current = nextDrafts
      setDrafts(nextDrafts)
      setDirty(cleanDrafts)
      setDraftSessionId(sessionId)
      setMutationError(undefined)
      return
    }
    if (!projection) return
    setDrafts((current) => ({
      goal: dirty.goal ? current.goal : projection.goal,
      plan: dirty.plan ? current.plan : projection.plan,
      recap: dirty.recap ? current.recap : projection.recap,
      notes: dirty.notes ? current.notes : projection.notes,
    }))
  }, [dirty, draftSessionId, query.data, sessionId])

  const apply = useCallback((next: ThreadWorkspaceMutation) => {
    if (!input.sessionId || mutation.isPending) return
    mutation.mutate(next)
  }, [input.sessionId, mutation])

  const setDraft = useCallback((field: DraftField, value: string) => {
    draftsRef.current = { ...draftsRef.current, [field]: value }
    setDrafts((current) => ({ ...current, [field]: value }))
    setDirty((current) => ({ ...current, [field]: true }))
  }, [])

  const save = useCallback((field: DraftField) => {
    const value = drafts[field]
    const kind = field === "goal" ? "goalSet" : field === "plan" ? "planSet" : field === "recap" ? "recapSet" : "notesSet"
    apply({ kind, value })
  }, [apply, drafts])

  return {
    sessionId: input.sessionId,
    projection: query.data,
    loading: query.isLoading,
    error: query.isError ? "Thread workspace could not be loaded." : undefined,
    drafts,
    dirty,
    busy: mutation.isPending,
    mutationError,
    setDraft,
    save,
    addPin: (runId, cursor) => apply({ kind: "pinAdded", pin: { runId, cursor: { sequence: cursor } } }),
    removePin: (cursor) => apply({ kind: "pinRemoved", cursor: { sequence: cursor } }),
    refresh: () => { void query.refetch() },
  }
}
