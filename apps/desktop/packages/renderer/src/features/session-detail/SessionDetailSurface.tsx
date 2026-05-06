import type { SessionId } from "@taugentic/desktop-shared";

import { Separator } from "@/components/ui/separator";
import { ApprovalInbox } from "@/features/approval-inbox";

import { AgentTurnsSection } from "./AgentTurnsSection";
import { ArtifactsSection } from "./ArtifactsSection";
import { RunTreeSection } from "../run-tree";
import { RunsSection } from "./RunsSection";

export interface SessionDetailSurfaceProps {
  sessionId: SessionId | null;
  /** Forwarded when a run is successfully started from the inline composer. */
  onRunStarted?: () => void;
}

export function SessionDetailSurface({ onRunStarted, sessionId }: SessionDetailSurfaceProps) {
  if (sessionId === null) {
    return (
      <div
        className="flex h-full w-full items-start"
        data-session-detail="empty"
        role="region"
        aria-label="Session detail"
      >
        <div className="px-3 py-3 font-[var(--font-mono)] text-[12px] text-[var(--fg-dim)]">
          Select a session in the left rail, or open a new one via the composer.
        </div>
      </div>
    );
  }

  return (
    <SessionDetailSurfaceBound key={sessionId} onRunStarted={onRunStarted} sessionId={sessionId} />
  );
}

function SessionDetailSurfaceBound({
  onRunStarted,
  sessionId,
}: {
  onRunStarted?: () => void;
  sessionId: SessionId;
}) {
  return (
    <div
      className="flex h-full w-full flex-col"
      data-session-detail="bound"
      data-session-id={sessionId}
      role="region"
      aria-label="Session detail"
    >
      <RunsSection onRunStarted={onRunStarted} sessionId={sessionId} />
      <Separator />
      <RunTreeSection sessionId={sessionId} />
      <Separator />
      <AgentTurnsSection sessionId={sessionId} />
      <Separator />
      <ApprovalInbox sessionId={sessionId} />
      <Separator />
      <ArtifactsSection sessionId={sessionId} />
    </div>
  );
}
