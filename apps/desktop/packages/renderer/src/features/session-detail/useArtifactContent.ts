import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type {
  ArtifactId,
  ArtifactSummary,
  ReadArtifactContentResult,
  SaveArtifactAsResult,
  SessionId,
} from "@taugentic/desktop-shared";

import { readArtifactContent, saveArtifactAs } from "@/lib/ipc/api";
import { queryKeys, sessionOverviewRootKey } from "@/lib/queries/keys";

function artifactContentKey(sessionId: SessionId, artifactId: ArtifactId): readonly unknown[] {
  return ["session", sessionId, "artifactContent", artifactId] as const;
}

/**
 * Fetches artifact content for the selected artifact via the D0 main-owned
 * `readArtifactContent` IPC. Returned data includes explicit `missing` /
 * `tooLarge` / `inline` statuses so the viewer can render the right fallback.
 *
 * Only enabled when an artifact is explicitly selected.
 */
export function useArtifactContentQuery(sessionId: SessionId, artifact: ArtifactSummary | null) {
  return useQuery<ReadArtifactContentResult>({
    enabled: artifact !== null,
    queryKey:
      artifact === null
        ? (["session", sessionId, "artifactContent", "__none__"] as const)
        : artifactContentKey(sessionId, artifact.id),
    queryFn: () =>
      readArtifactContent({
        sessionId,
        artifactId: artifact!.id,
      }),
    // Artifact files are immutable per id, so the cached payload stays fresh
    // until the artifact itself is replaced.
    staleTime: Infinity,
  });
}

/**
 * Opens a native Save dialog (via D0 main-owned `saveArtifactAs`) and copies
 * the daemon-owned file to the user-chosen path.
 *
 * On `missing`, the shared artifact list + session overview queries are
 * invalidated automatically so stale rows refresh without each caller
 * having to remember to wire that up.
 */
export function useSaveArtifactAsMutation(sessionId: SessionId) {
  const qc = useQueryClient();
  return useMutation<SaveArtifactAsResult, Error, { artifact: ArtifactSummary }>({
    mutationFn: ({ artifact }) =>
      saveArtifactAs({
        sessionId,
        artifactId: artifact.id,
        suggestedFilename: defaultArtifactFilename(artifact),
      }),
    onSuccess: (result) => {
      if (result.status === "missing") {
        void qc.invalidateQueries({ queryKey: queryKeys.sessionArtifacts(sessionId) });
        void qc.invalidateQueries({ queryKey: sessionOverviewRootKey });
      }
    },
  });
}

/**
 * Derives a reasonable Save-dialog default filename from an `ArtifactSummary`.
 *
 * Used by both the renderer (to pass `suggestedFilename` to main) and exposed
 * for unit tests.
 */
export function defaultArtifactFilename(artifact: ArtifactSummary): string {
  const extension = artifactKindExtension(artifact.kind);
  return `${artifact.id}${extension}`;
}

function artifactKindExtension(kind: ArtifactSummary["kind"]): string {
  switch (kind) {
    case "Patch":
      return ".diff";
    case "CommandLog":
      return ".log";
    case "Transcript":
      return ".md";
    case "FileSnapshot":
      return ".txt";
  }
}
