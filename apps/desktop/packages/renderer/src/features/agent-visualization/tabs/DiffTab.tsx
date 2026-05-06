/*
 * DiffTab.
 *
 * Thin wrapper around the canonical ArtifactsSection. ArtifactSummary's
 * `kind` discriminator covers Patch / FileSnapshot / CommandLog /
 * Transcript, which is the existing place where diff-shaped artifacts
 * surface. No re-implementation here.
 */

import type { SessionId } from "@taugentic/desktop-shared";

import { ArtifactsSection } from "@/features/session-detail";

export interface DiffTabProps {
  sessionId: SessionId;
}

export function DiffTab({ sessionId }: DiffTabProps) {
  return (
    <div className="flex flex-col" data-agent-visualization-tab="diff">
      <ArtifactsSection sessionId={sessionId} />
    </div>
  );
}
