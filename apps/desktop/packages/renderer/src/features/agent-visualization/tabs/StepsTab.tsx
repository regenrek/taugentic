/*
 * StepsTab.
 *
 * Mission Control "steps" surface.
 *
 * The primary working area is the canonical `AgentTurnsSection`, given the
 * available vertical space in the center column. The run composer moves to
 * the bottom by using `RunsSection composerPlacement=\"bottom\"`, so the
 * operator sees turns first and composes the next run at the bottom edge.
 */

import type { SessionId } from "@taugentic/desktop-shared";

import { AgentTurnsSection, ApprovalsSection, RunsSection } from "@/features/session-detail";

export interface StepsTabProps {
  sessionId: SessionId;
  onRunStarted?: () => void;
}

export function StepsTab({ onRunStarted, sessionId }: StepsTabProps) {
  return (
    <div className="flex min-h-0 flex-1 flex-col" data-agent-visualization-tab="steps">
      <ApprovalsSection hideWhenEmpty sessionId={sessionId} variant="mission-control" />
      <AgentTurnsSection
        className="min-h-0 flex-1"
        sessionId={sessionId}
        variant="mission-control"
      />
      <RunsSection composerPlacement="bottom" onRunStarted={onRunStarted} sessionId={sessionId} />
    </div>
  );
}
