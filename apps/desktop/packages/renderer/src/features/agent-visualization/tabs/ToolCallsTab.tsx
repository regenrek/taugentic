/*
 * ToolCallsTab.
 *
 * Thin wrapper around the canonical ApprovalsSection, which is the
 * existing surface for tool-call human-in-the-loop decisions. No
 * re-implementation: the section owns its own header, empty state,
 * error state and approve/reject mutation wiring.
 */

import type { SessionId } from "@taugentic/desktop-shared";

import { ApprovalsSection } from "@/features/session-detail";

export interface ToolCallsTabProps {
  sessionId: SessionId;
}

export function ToolCallsTab({ sessionId }: ToolCallsTabProps) {
  return (
    <div className="flex flex-col" data-agent-visualization-tab="tools">
      <ApprovalsSection sessionId={sessionId} />
    </div>
  );
}
