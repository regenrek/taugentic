import { useQuery } from "@tanstack/react-query"
import { useEffect, useMemo, useState } from "react"

import type { ArtifactId, SessionId } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { artifactContentQuery, artifactImagePreviewQuery, artifactsListQuery } from "../../platform/daemon/artifacts-query.js"

const NO_SESSION = "session-not-selected" as SessionId
const NO_ARTIFACT = "artifact-not-selected" as ArtifactId

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
}

export function useWorkbenchArtifacts(input: { runtime: DesktopRuntime; sessionId?: SessionId; enabled: boolean }) {
  const sessionId = input.sessionId ?? NO_SESSION
  const [selectedArtifactId, setSelectedArtifactId] = useState<ArtifactId>()
  const [pdfPageIndex, setPdfPageIndex] = useState(0)
  const listQuery = useQuery({
    ...artifactsListQuery(input.runtime, sessionId),
    enabled: input.enabled && Boolean(input.sessionId),
  })
  const selectedArtifact = useMemo(
    () => listQuery.data?.items.find((item) => item.id === selectedArtifactId),
    [listQuery.data?.items, selectedArtifactId],
  )
  const contentQuery = useQuery({
    ...artifactContentQuery(input.runtime, sessionId, selectedArtifactId ?? NO_ARTIFACT, pdfPageIndex),
    enabled: input.enabled && Boolean(input.sessionId) && Boolean(selectedArtifactId) && selectedArtifact?.kind !== "Image",
  })
  const imagePreviewQuery = useQuery({
    ...artifactImagePreviewQuery(input.runtime, sessionId, selectedArtifactId ?? NO_ARTIFACT),
    enabled: input.enabled && Boolean(input.sessionId) && selectedArtifact?.kind === "Image",
  })

  useEffect(() => {
    setSelectedArtifactId(undefined)
    setPdfPageIndex(0)
  }, [sessionId])

  return {
    artifacts: listQuery.data?.items ?? [],
    loading: listQuery.isLoading,
    error: listQuery.isError ? errorMessage(listQuery.error, "Artifacts could not be loaded.") : undefined,
    selectedArtifact,
    selectedContent: contentQuery.data?.content,
    selectedImagePreview: imagePreviewQuery.data,
    contentLoading: selectedArtifact?.kind === "Image" ? imagePreviewQuery.isLoading : contentQuery.isLoading,
    contentError: selectedArtifact?.kind === "Image"
      ? imagePreviewQuery.isError ? errorMessage(imagePreviewQuery.error, "The image could not be loaded.") : undefined
      : contentQuery.isError ? errorMessage(contentQuery.error, "The artifact could not be loaded.") : undefined,
    selectArtifact: (artifactId: ArtifactId) => {
      setSelectedArtifactId(artifactId)
      setPdfPageIndex(0)
    },
    pdfPageIndex,
    setPdfPageIndex,
    refresh: () => { void listQuery.refetch() },
  }
}
