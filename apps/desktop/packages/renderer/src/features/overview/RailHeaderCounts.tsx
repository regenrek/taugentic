import { StatusDot } from "@/components/ui/status-dot";
import { cn } from "@/lib/ui/cn";

import type { OverviewLaneCounts } from "./formatters";

export interface RailHeaderCountsViewProps {
  className?: string;
  counts: OverviewLaneCounts;
}

export function RailHeaderCountsView({ className, counts }: RailHeaderCountsViewProps) {
  return (
    <div
      aria-label="Session lane counts"
      className={cn(
        "flex items-center gap-3 border-b border-[var(--border)] bg-[var(--bg-raised)] px-3 py-1.5 font-[var(--font-mono)] text-[10px] uppercase tracking-[0.16em] text-[var(--fg-dim)]",
        className,
      )}
      data-feature="rail-header-counts"
    >
      <RailCountChip count={counts.active} label="active" tone="active" />
      <span className="text-[var(--fg-mute)]">·</span>
      <RailCountChip count={counts.waiting} label="wait" tone="waiting" />
      <span className="text-[var(--fg-mute)]">·</span>
      <RailCountChip count={counts.failed} label="failed" tone="failed" />
    </div>
  );
}

function RailCountChip({
  count,
  label,
  tone,
}: {
  count: number;
  label: string;
  tone: "active" | "waiting" | "failed";
}) {
  return (
    <span
      className="inline-flex items-center gap-1.5"
      data-rail-count-chip={label}
      data-tone={tone}
    >
      <StatusDot tone={tone} />
      <span>{label}</span>
      <span className="text-[var(--fg-mute)]">·</span>
      <span className="tabular-nums text-[var(--fg)]">{count}</span>
    </span>
  );
}

export type { OverviewLaneCounts };
