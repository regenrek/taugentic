import { useQuery } from "@tanstack/react-query"
import { useCallback, useEffect, useMemo, useState } from "react"

import type { ProjectId, WorkspaceFileEntry, WorkspaceId } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { openWorkspaceFileExternal, workspaceFileReadQuery, workspaceFileTreeQuery, workspaceImagePreviewQuery, writeWorkspaceTextFile } from "../../platform/daemon/workspace-files-query.js"

const NO_PROJECT = "project-not-selected" as ProjectId
const NO_WORKSPACE = "workspace-not-selected" as WorkspaceId

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
}

export function useWorkbenchFiles(input: {
  runtime: DesktopRuntime
  projectId?: ProjectId
  workspaceId?: WorkspaceId
  enabled: boolean
}) {
  const projectId = input.projectId ?? NO_PROJECT
  const workspaceId = input.workspaceId ?? NO_WORKSPACE
  const [selectedPath, setSelectedPath] = useState<string>()
  const [draft, setDraft] = useState("")
  const [draftRevision, setDraftRevision] = useState<string>()
  const [draftPath, setDraftPath] = useState<string>()
  const [mutationError, setMutationError] = useState<string>()
  const [saving, setSaving] = useState(false)
  const [pdfPageIndex, setPdfPageIndex] = useState(0)
  const treeQuery = useQuery({
    ...workspaceFileTreeQuery(input.runtime, projectId, workspaceId),
    enabled: input.enabled && Boolean(input.projectId) && Boolean(input.workspaceId),
  })
  const selectedEntry = useMemo(
    () => treeQuery.data?.entries.find((entry) => entry.path === selectedPath),
    [selectedPath, treeQuery.data?.entries],
  )
  const readableSelection = selectedEntry && selectedEntry.kind !== "directory" && !selectedEntry.isSymlink
    ? selectedEntry.path
    : undefined
  const contentQuery = useQuery({
    ...workspaceFileReadQuery(
      input.runtime,
      projectId,
      workspaceId,
      readableSelection ?? "file-not-selected",
      selectedEntry?.kind === "pdf" ? pdfPageIndex : undefined,
    ),
    enabled: input.enabled && Boolean(readableSelection) && selectedEntry?.kind !== "image",
  })
  const imagePreviewQuery = useQuery({
    ...workspaceImagePreviewQuery(input.runtime, projectId, workspaceId, selectedEntry?.kind === "image" ? selectedEntry.path : "image-not-selected"),
    enabled: input.enabled && selectedEntry?.kind === "image",
  })

  useEffect(() => {
    setSelectedPath(undefined)
    setDraft("")
    setDraftRevision(undefined)
    setDraftPath(undefined)
    setMutationError(undefined)
    setPdfPageIndex(0)
  }, [projectId, workspaceId])

  useEffect(() => {
    const result = contentQuery.data
    if (!result || result.content.kind !== "text") {
      if (result) {
        setDraft("")
        setDraftRevision(undefined)
        setDraftPath(undefined)
      }
      return
    }
    if (draftPath === result.path && draftRevision === result.content.revision) return
    setDraft(result.content.text)
    setDraftRevision(result.content.revision)
    setDraftPath(result.path)
    setMutationError(undefined)
  }, [contentQuery.data, draftPath, draftRevision])

  const selectEntry = useCallback((entry: WorkspaceFileEntry) => {
    if (entry.kind === "directory" || entry.isSymlink) return
    setSelectedPath(entry.path)
    setPdfPageIndex(0)
    setMutationError(undefined)
  }, [])

  const save = useCallback(async () => {
    if (!selectedPath || !draftRevision || !input.projectId || !input.workspaceId) return
    setSaving(true)
    setMutationError(undefined)
    try {
      const result = await writeWorkspaceTextFile(input.runtime, {
        projectId: input.projectId,
        workspaceId: input.workspaceId,
        path: selectedPath,
        expectedRevision: draftRevision,
        text: draft,
      })
      setDraftRevision(result.revision)
    } catch (error) {
      setMutationError(errorMessage(error, "The file could not be saved."))
    } finally {
      setSaving(false)
    }
  }, [draft, draftRevision, input.projectId, input.runtime, input.workspaceId, selectedPath])

  const discard = useCallback(() => {
    const content = contentQuery.data?.content
    if (content?.kind !== "text") return
    setDraft(content.text)
    setDraftRevision(content.revision)
    setMutationError(undefined)
  }, [contentQuery.data?.content])

  const openExternal = useCallback(async () => {
    if (!selectedPath || !input.projectId || !input.workspaceId) return
    setMutationError(undefined)
    try {
      await openWorkspaceFileExternal(input.runtime, {
        projectId: input.projectId,
        workspaceId: input.workspaceId,
        path: selectedPath,
      })
    } catch (error) {
      setMutationError(errorMessage(error, "The file could not be opened externally."))
    }
  }, [input.projectId, input.runtime, input.workspaceId, selectedPath])

  const source = contentQuery.data?.content
  const dirty = source?.kind === "text" && draftPath === contentQuery.data?.path && draft !== source.text
  return {
    entries: treeQuery.data?.entries ?? [],
    treeTruncated: treeQuery.data?.truncated ?? false,
    treeLoading: treeQuery.isLoading,
    treeError: treeQuery.isError ? errorMessage(treeQuery.error, "The project files could not be loaded.") : undefined,
    selectedPath,
    selectedEntry,
    selectedContent: contentQuery.data?.content,
    selectedImagePreview: imagePreviewQuery.data,
    contentLoading: selectedEntry?.kind === "image" ? imagePreviewQuery.isLoading : contentQuery.isLoading,
    contentError: selectedEntry?.kind === "image"
      ? imagePreviewQuery.isError ? errorMessage(imagePreviewQuery.error, "The image could not be loaded.") : mutationError
      : contentQuery.isError ? errorMessage(contentQuery.error, "The file could not be loaded.") : mutationError,
    draft,
    dirty,
    saving,
    selectEntry,
    setDraft,
    save,
    discard,
    openExternal,
    pdfPageIndex,
    setPdfPageIndex,
    refreshTree: () => { void treeQuery.refetch() },
  }
}
