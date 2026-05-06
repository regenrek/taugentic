/*
 * MetricsTab.
 *
 * Thin wrapper around the canonical MetricsSection. MetricsSection was
 * extracted into features/session-detail/ in this slice because the
 * pre-existing surface lacked a discrete metrics sub-component; that
 * extraction keeps `agent-visualization/` presentation-only and avoids
 * duplicating any aggregation or formatting logic here.
 */

import type { SessionId } from "@taugentic/desktop-shared";

import { MetricsSection } from "@/features/session-detail";

export interface MetricsTabProps {
  sessionId: SessionId;
}

export function MetricsTab({ sessionId }: MetricsTabProps) {
  return (
    <div className="flex flex-col" data-agent-visualization-tab="metrics">
      <MetricsSection sessionId={sessionId} />
    </div>
  );
}
