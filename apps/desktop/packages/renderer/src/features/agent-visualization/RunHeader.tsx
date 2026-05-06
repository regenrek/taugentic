/*
 * RunHeader.
 *
 * Compact bar above the cortex field showing the focused session id, the
 * latest run status (read from `useSessionRunsQuery`), and the manual
 * cortex pause toggle (wired to motion.store via the xstate-store
 * selector hook).
 *
 * Real model / token / cost numbers are not yet exposed by the daemon for
 * a focused run; we surface session id and run status as the canonical
 * "what am I looking at" labels until that contract lands. No new domain
 * owner is introduced here -- only existing TanStack Query hooks are read.
 */

import { useSelector } from "@xstate/store/react";

import type { SessionId } from "@taugentic/desktop-shared";

import { Toggle } from "@/components/ui/toggle";
import { useSessionRunsQuery } from "@/lib/queries/session-queries";

import { pickLatestRun } from "./selectors/focused-run";
import { motionStore, selectMotionPaused, toggleMotionPaused } from "./state/motion.store";

export interface RunHeaderProps {
  sessionId: SessionId;
}

export function RunHeader({ sessionId }: RunHeaderProps) {
  const runsQuery = useSessionRunsQuery(sessionId);
  const latest = pickLatestRun(runsQuery.data);
  const paused = useSelector(motionStore, selectMotionPaused);

  return (
    <header
      aria-label="Run header"
      className="flex items-center justify-between gap-3 border-b border-[var(--border)] bg-[var(--bg-raised)] px-3 py-2 font-[var(--font-mono)] text-[11px] text-[var(--fg)]"
      data-agent-visualization-run-header
    >
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <RunHeaderField label="session" value={sessionId} mono mute={false} />
        <RunHeaderField
          label="run"
          mono
          mute={latest === null}
          value={latest === null ? "—" : latest.objective}
        />
        <RunHeaderField
          label="status"
          mono={false}
          mute={latest === null}
          value={latest === null ? "no runs" : latest.status}
        />
      </div>
      <Toggle
        aria-label={paused ? "Resume cortex motion" : "Pause cortex motion"}
        data-agent-visualization-pause-toggle
        defaultPressed={paused}
        onPressedChange={() => toggleMotionPaused()}
        pressed={paused}
        size="sm"
        variant="outline"
      >
        {paused ? "RESUME" : "PAUSE"}
      </Toggle>
    </header>
  );
}

function RunHeaderField({
  label,
  mono,
  mute,
  value,
}: {
  label: string;
  mono: boolean;
  mute: boolean;
  value: string;
}) {
  return (
    <div className="flex min-w-0 items-center gap-1.5" data-run-header-field={label}>
      <span className="shrink-0 text-[10px] uppercase tracking-[0.18em] text-[var(--fg-dim)]">
        {label}
      </span>
      <span
        className={`truncate ${mono ? "font-[var(--font-mono)]" : ""} ${mute ? "text-[var(--fg-mute)]" : "text-[var(--fg)]"}`}
        title={value}
      >
        {value}
      </span>
    </div>
  );
}
