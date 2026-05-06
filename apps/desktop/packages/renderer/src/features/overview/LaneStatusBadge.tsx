import type { SessionOverviewLaneStatus } from "@taugentic/desktop-shared";

import { StatusDot, type StatusTone } from "@/components/ui/status-dot";
import { cn } from "@/lib/ui/cn";

import { describeLaneStatus, type OverviewLaneTone } from "./formatters";

export interface LaneStatusBadgeProps {
  className?: string;
  laneStatus: SessionOverviewLaneStatus;
  showLabel?: boolean;
}

/**
 * Compact lane-status indicator for SessionRail rows.
 *
 * Pure derivation from {@link describeLaneStatus}; no internal data ownership.
 */
export function LaneStatusBadge({ className, laneStatus, showLabel = true }: LaneStatusBadgeProps) {
  const presentation = describeLaneStatus(laneStatus);
  const tone = toStatusTone(presentation.tone);

  return (
    <span
      aria-label={`Lane status: ${presentation.label}`}
      className={cn(
        "inline-flex items-center gap-1.5 font-[var(--font-mono)] text-[10px] uppercase tracking-[0.16em] text-[var(--fg-dim)]",
        className,
      )}
      data-feature="lane-status-badge"
      data-lane-status={laneStatus}
      data-tone={tone}
      role="status"
    >
      <StatusDot tone={tone} />
      {showLabel ? <span data-lane-status-label>{presentation.label}</span> : null}
    </span>
  );
}

function toStatusTone(tone: OverviewLaneTone): StatusTone {
  switch (tone) {
    case "active":
    case "waiting":
    case "failed":
    case "completed":
    case "cancelled":
    case "idle":
      return tone;
    default:
      return "idle";
  }
}
