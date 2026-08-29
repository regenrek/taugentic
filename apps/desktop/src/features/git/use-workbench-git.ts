import { useQuery } from "@tanstack/react-query"
import type {
  GitCheckpointPrepareRevertResult,
  GitDiffScope,
  GitFileStatus,
  ProjectId,
  RunStatus,
  WorkspaceId,
} from "@taugentic/desktop-protocol"
import { useCallback, useEffect, useMemo, useState } from "react"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import {
  applyGitCheckpointRevert,
  gitCheckpointsQuery,
  gitCommit,
  gitDiffQuery,
  gitSnapshotQuery,
  gitStage,
  gitUnstage,
  invalidateGitAfterRun,
  prepareGitCheckpointRevert,
} from "../../platform/daemon/git-query.js"

const NO_PROJECT = "project-not-selected" as ProjectId
const NO_WORKSPACE = "workspace-not-selected" as WorkspaceId

export type GitDiffView = "unstaged" | "staged" | "lastTurn"

function diffScope(view: GitDiffView): GitDiffScope {
  if (view === "staged") return { kind: "staged" }
  if (view === "lastTurn") return { kind: "lastTurn" }
  return { kind: "unstaged" }
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
}

function isTerminalRunStatus(status?: RunStatus): boolean {
  return status === "completed" || status === "failed" || status === "cancelled" || status === "budgetExceeded"
}

export function useWorkbenchGit(input: {
  runtime: DesktopRuntime
  projectId?: ProjectId
  workspaceId?: WorkspaceId
  enabled: boolean
  runStatus?: RunStatus
}) {
  const projectId = input.projectId ?? NO_PROJECT
  const workspaceId = input.workspaceId ?? NO_WORKSPACE
  const queryEnabled = input.enabled && Boolean(input.projectId) && Boolean(input.workspaceId)
  const [view, setView] = useState<GitDiffView>("unstaged")
  const [selectedPaths, setSelectedPaths] = useState<readonly string[]>([])
  const [commitMessage, setCommitMessage] = useState("")
  const [busy, setBusy] = useState(false)
  const [mutationError, setMutationError] = useState<string>()
  const [preparedRevert, setPreparedRevert] = useState<GitCheckpointPrepareRevertResult>()

  const snapshotQuery = useQuery({
    ...gitSnapshotQuery(input.runtime, projectId, workspaceId),
    enabled: queryEnabled,
  })
  const diffQuery = useQuery({
    ...gitDiffQuery(input.runtime, projectId, workspaceId, diffScope(view)),
    enabled: queryEnabled,
  })
  const checkpointsQuery = useQuery({
    ...gitCheckpointsQuery(input.runtime, projectId, workspaceId),
    enabled: queryEnabled,
  })

  useEffect(() => {
    setView("unstaged")
    setSelectedPaths([])
    setCommitMessage("")
    setMutationError(undefined)
    setPreparedRevert(undefined)
  }, [projectId, workspaceId])

  useEffect(() => {
    if (!input.projectId || !input.workspaceId || !isTerminalRunStatus(input.runStatus)) return
    void invalidateGitAfterRun(input.projectId, input.workspaceId)
  }, [input.projectId, input.runStatus, input.workspaceId])

  const visibleFiles = useMemo<readonly GitFileStatus[]>(() => {
    const files = snapshotQuery.data?.snapshot.files ?? []
    if (view === "staged") return files.filter((file) => Boolean(file.staged))
    if (view === "unstaged") return files.filter((file) => Boolean(file.unstaged))
    return files
  }, [snapshotQuery.data?.snapshot.files, view])

  useEffect(() => {
    const visible = new Set(visibleFiles.map((file) => file.path))
    setSelectedPaths((current) => current.filter((path) => visible.has(path)))
  }, [visibleFiles])

  const togglePath = useCallback((path: string) => {
    setSelectedPaths((current) => current.includes(path)
      ? current.filter((candidate) => candidate !== path)
      : [...current, path])
  }, [])

  const perform = useCallback(async (operation: () => Promise<void>, fallback: string) => {
    setBusy(true)
    setMutationError(undefined)
    try {
      await operation()
    } catch (error) {
      setMutationError(errorMessage(error, fallback))
    } finally {
      setBusy(false)
    }
  }, [])

  const stageSelected = useCallback(() => {
    if (!input.projectId || !input.workspaceId || !selectedPaths.length) return
    void perform(async () => {
      await gitStage(input.runtime, { projectId: input.projectId!, workspaceId: input.workspaceId!, paths: [...selectedPaths] })
      setSelectedPaths([])
    }, "The selected files could not be staged.")
  }, [input.projectId, input.runtime, input.workspaceId, perform, selectedPaths])

  const unstageSelected = useCallback(() => {
    if (!input.projectId || !input.workspaceId || !selectedPaths.length) return
    void perform(async () => {
      await gitUnstage(input.runtime, { projectId: input.projectId!, workspaceId: input.workspaceId!, paths: [...selectedPaths] })
      setSelectedPaths([])
    }, "The selected files could not be unstaged.")
  }, [input.projectId, input.runtime, input.workspaceId, perform, selectedPaths])

  const commit = useCallback(() => {
    if (!input.projectId || !input.workspaceId || !commitMessage.trim()) return
    void perform(async () => {
      await gitCommit(input.runtime, {
        projectId: input.projectId!,
        workspaceId: input.workspaceId!,
        message: commitMessage,
      })
      setCommitMessage("")
    }, "The commit could not be created.")
  }, [commitMessage, input.projectId, input.runtime, input.workspaceId, perform])

  const prepareRevert = useCallback((checkpointId: string) => {
    if (!input.projectId || !input.workspaceId) return
    void perform(async () => {
      const prepared = await prepareGitCheckpointRevert(input.runtime, {
        projectId: input.projectId!,
        workspaceId: input.workspaceId!,
        checkpointId,
      })
      setPreparedRevert(prepared)
    }, "The checkpoint preview could not be prepared.")
  }, [input.projectId, input.runtime, input.workspaceId, perform])

  const applyRevert = useCallback(() => {
    if (!input.projectId || !input.workspaceId || !preparedRevert) return
    void perform(async () => {
      await applyGitCheckpointRevert(
        input.runtime,
        { token: preparedRevert.token },
        input.projectId!,
        input.workspaceId!,
      )
      setPreparedRevert(undefined)
      setSelectedPaths([])
    }, "The checkpoint could not be restored.")
  }, [input.projectId, input.runtime, input.workspaceId, perform, preparedRevert])

  const refresh = useCallback(() => {
    setMutationError(undefined)
    void Promise.all([snapshotQuery.refetch(), diffQuery.refetch(), checkpointsQuery.refetch()])
  }, [checkpointsQuery, diffQuery, snapshotQuery])

  return {
    snapshot: snapshotQuery.data?.snapshot,
    visibleFiles,
    view,
    setView,
    selectedPaths,
    togglePath,
    patch: preparedRevert?.patch ?? diffQuery.data?.patch ?? "",
    patchLoading: diffQuery.isLoading,
    preparedRevert,
    cancelRevert: () => setPreparedRevert(undefined),
    checkpoints: checkpointsQuery.data?.checkpoints ?? [],
    commitMessage,
    setCommitMessage,
    busy,
    loading: snapshotQuery.isLoading,
    error: mutationError
      ?? (snapshotQuery.isError ? errorMessage(snapshotQuery.error, "Git status could not be loaded.") : undefined)
      ?? (diffQuery.isError ? errorMessage(diffQuery.error, "The diff could not be loaded.") : undefined)
      ?? (checkpointsQuery.isError ? errorMessage(checkpointsQuery.error, "Checkpoints could not be loaded.") : undefined),
    canStage: view === "unstaged" && selectedPaths.length > 0 && !busy,
    canUnstage: view === "staged" && selectedPaths.length > 0 && !busy,
    canCommit: Boolean(commitMessage.trim()) && Boolean(snapshotQuery.data?.snapshot.files.some((file) => file.staged)) && !busy,
    stageSelected,
    unstageSelected,
    commit,
    prepareRevert,
    applyRevert,
    refresh,
  }
}

export type WorkbenchGitState = ReturnType<typeof useWorkbenchGit>
