import { queryOptions } from "@tanstack/react-query"

import type { ArtifactContentResult, ArtifactId, ArtifactSnapshotResult, NativeImagePreview, SessionId } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"
import { decodeProtocolJson } from "./protocol-json.js"

export const artifactsQueryRoot = ["daemon", "artifacts"] as const

export function artifactsListQueryKey(sessionId: SessionId) {
  return [...artifactsQueryRoot, sessionId, "list"] as const
}

export function artifactContentQueryKey(sessionId: SessionId, artifactId: ArtifactId, pdfPageIndex?: number) {
  return [...artifactsQueryRoot, sessionId, "content", artifactId, pdfPageIndex ?? "content"] as const
}

export function artifactImagePreviewQueryKey(sessionId: SessionId, artifactId: ArtifactId) {
  return [...artifactsQueryRoot, sessionId, "image-preview", artifactId] as const
}

export function artifactsListQuery(runtime: DesktopRuntime, sessionId: SessionId) {
  return queryOptions({
    queryKey: artifactsListQueryKey(sessionId),
    queryFn: async (): Promise<ArtifactSnapshotResult> => decodeProtocolJson(
      await runtime.bridge.listArtifacts(sessionId, JSON.stringify({})),
    ),
  })
}

export function artifactContentQuery(runtime: DesktopRuntime, sessionId: SessionId, artifactId: ArtifactId, pdfPageIndex?: number) {
  return queryOptions({
    queryKey: artifactContentQueryKey(sessionId, artifactId, pdfPageIndex),
    queryFn: async (): Promise<ArtifactContentResult> => decodeProtocolJson(
      await runtime.bridge.getArtifact(sessionId, JSON.stringify({ artifactId, ...(pdfPageIndex === undefined ? {} : { pdfPageIndex }) })),
    ),
  })
}

export function artifactImagePreviewQuery(runtime: DesktopRuntime, sessionId: SessionId, artifactId: ArtifactId) {
  return queryOptions({
    queryKey: artifactImagePreviewQueryKey(sessionId, artifactId),
    queryFn: async (): Promise<NativeImagePreview> => decodeProtocolJson(
      await runtime.bridge.materializeArtifactImage(sessionId, JSON.stringify({ artifactId })),
    ),
  })
}
