import type { ArtifactId, ArtifactSummary } from "@taugentic/desktop-shared";

export function reconcileCurrentArtifactId(
  currentArtifactId: ArtifactId | null,
  artifacts: ArtifactSummary[],
): ArtifactId | null {
  if (currentArtifactId === null) {
    return null;
  }

  return artifacts.some((artifact) => artifact.id === currentArtifactId) ? currentArtifactId : null;
}
