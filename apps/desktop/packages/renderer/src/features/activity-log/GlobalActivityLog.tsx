import { useMemo, useState } from "react";

import type { PublicDaemonEventEnvelope } from "@taugentic/desktop-shared";

import { Popover } from "@/components/ui/popover";

import { useSessionOverviewQuery } from "@/lib/queries/session-queries";

import {
  eventKind,
  formatEventSummary,
  mergeRecentActivity,
  type ActivityEventKind,
} from "./formatters";

export type ActivityKindFilter = "all" | "run" | "approval" | "artifact" | "session";

const KIND_FILTER_OPTIONS: readonly ActivityKindFilter[] = [
  "all",
  "run",
  "approval",
  "artifact",
  "session",
] as const;

export interface GlobalActivityLogViewState {
  activeKindFilter: ActivityKindFilter;
  errorMessage: string | null;
  events: PublicDaemonEventEnvelope[];
  hasLoaded: boolean;
}

export function createInitialGlobalActivityLogViewState(): GlobalActivityLogViewState {
  return {
    activeKindFilter: "all",
    errorMessage: null,
    events: [],
    hasLoaded: false,
  };
}

export function GlobalActivityLog() {
  const query = useSessionOverviewQuery();
  const [activeKindFilter, setActiveKindFilter] = useState<ActivityKindFilter>("all");

  const events = useMemo(
    () => (query.data === undefined ? [] : mergeRecentActivity(query.data)),
    [query.data],
  );

  const hasLoaded = query.data !== undefined;
  const errorMessage = query.error ? toErrorMessage(query.error) : null;

  return (
    <GlobalActivityLogView
      nowMs={Date.now()}
      onKindFilterChange={setActiveKindFilter}
      state={{ activeKindFilter, errorMessage, events, hasLoaded }}
    />
  );
}

export interface GlobalActivityLogViewProps {
  nowMs: number;
  onKindFilterChange: (filter: ActivityKindFilter) => void;
  state: GlobalActivityLogViewState;
}

export function GlobalActivityLogView({
  nowMs,
  onKindFilterChange,
  state,
}: GlobalActivityLogViewProps) {
  const visibleEvents = filterEventsByKind(state.events, state.activeKindFilter);

  const hasInitialError = state.errorMessage !== null && !state.hasLoaded;
  const hasStaleError = state.errorMessage !== null && state.hasLoaded;
  const isEmpty = !hasInitialError && state.hasLoaded && visibleEvents.length === 0;

  return (
    <section
      aria-label="Activity log"
      className="flex h-full flex-col bg-[var(--bg)] text-[var(--fg)] font-[var(--font-mono)]"
      data-feature="global-activity-log"
    >
      <header className="flex items-center justify-between border-b border-[var(--border)] px-3 py-2">
        <span className="text-[10px] uppercase tracking-[0.18em] text-[var(--fg-mute)]">
          ACTIVITY
        </span>
        <Popover.Root>
          <Popover.Trigger
            className="inline-flex items-center gap-1 border border-[var(--border)] bg-transparent px-2 py-[2px] text-[10px] uppercase tracking-[0.16em] text-[var(--fg)]"
            data-activity-kind-filter-trigger
            type="button"
          >
            <span className="text-[var(--fg-mute)]">KIND</span>
            <span>{state.activeKindFilter}</span>
          </Popover.Trigger>
          <Popover.Content
            align="end"
            className="flex min-w-[8rem] flex-col gap-0 p-1"
            side="bottom"
          >
            {KIND_FILTER_OPTIONS.map((option) => (
              <button
                key={option}
                className="flex items-center justify-between gap-4 border border-transparent px-2 py-1 text-left text-[10px] uppercase tracking-[0.16em] text-[var(--fg)] hover:bg-[var(--border)]"
                data-activity-kind-filter-option={option}
                data-activity-kind-filter-active={
                  option === state.activeKindFilter ? "true" : "false"
                }
                onClick={() => onKindFilterChange(option)}
                type="button"
              >
                <span>{option}</span>
                {option === state.activeKindFilter ? (
                  <span className="text-[var(--fg-mute)]">·</span>
                ) : null}
              </button>
            ))}
          </Popover.Content>
        </Popover.Root>
      </header>

      {hasStaleError ? (
        <div
          className="border-b border-[var(--status-failed)]/40 bg-[var(--bg-raised)] px-3 py-1.5 text-[11px] text-[var(--status-failed)]"
          data-state="stale"
        >
          stale · last refresh failed: {state.errorMessage}
        </div>
      ) : null}
      {hasInitialError ? (
        <div
          className="m-3 border border-[var(--status-failed)] bg-transparent px-3 py-2 text-[12px] text-[var(--status-failed)]"
          data-state="error"
        >
          Daemon event stream unavailable: {state.errorMessage}
        </div>
      ) : isEmpty ? (
        <div className="px-3 py-4 text-[12px] text-[var(--fg-dim)]" data-state="empty">
          No daemon events yet.
        </div>
      ) : !hasInitialError ? (
        <ol className="flex flex-col" data-state="ready">
          {visibleEvents.map((envelope) => (
            <ActivityRow
              key={`${envelope.daemonInstanceId}|${envelope.sessionId}|${envelope.sequence.toString()}`}
              envelope={envelope}
              nowMs={nowMs}
            />
          ))}
        </ol>
      ) : null}
    </section>
  );
}

function ActivityRow({ envelope, nowMs }: { envelope: PublicDaemonEventEnvelope; nowMs: number }) {
  const kind = eventKind(envelope);
  const kindColor = kindColorVar(kind);
  const relative = formatRelativeTimeMs(envelope.occurredAtMs, nowMs);
  const sessionShort = shortenSessionId(envelope.sessionId);
  const summary = formatEventSummary(envelope);

  return (
    <li
      className="mc-phosphor-decay flex items-baseline gap-2 border-b border-[var(--border)] px-3 py-1.5"
      data-kind={kind}
      data-session-id={envelope.sessionId}
    >
      <time className="shrink-0 text-[10px] text-[var(--fg-mute)] tabular-nums">{relative}</time>
      <span className="shrink-0 text-[10px] text-[var(--fg-dim)] tabular-nums">{sessionShort}</span>
      <span
        className="shrink-0 text-[10px] uppercase tracking-[0.16em]"
        data-activity-kind-label
        style={{ color: kindColor }}
      >
        {kind}
      </span>
      <span
        className="min-w-0 flex-1 overflow-hidden whitespace-nowrap text-ellipsis text-[11px] text-[var(--fg)]"
        title={summary}
      >
        {summary}
      </span>
    </li>
  );
}

function filterEventsByKind(
  events: PublicDaemonEventEnvelope[],
  filter: ActivityKindFilter,
): PublicDaemonEventEnvelope[] {
  if (filter === "all") {
    return events;
  }
  return events.filter((envelope) => eventKind(envelope) === filter);
}

function kindColorVar(kind: ActivityEventKind): string {
  switch (kind) {
    case "run":
      return "var(--accent)";
    case "approval":
      return "var(--status-waiting)";
    case "artifact":
      return "var(--status-completed)";
    case "session":
    default:
      return "var(--fg-dim)";
  }
}

function shortenSessionId(sessionId: string): string {
  if (sessionId.length <= 8) {
    return sessionId;
  }
  return sessionId.slice(-8);
}

function formatRelativeTimeMs(occurredAtMs: bigint, nowMs: number): string {
  const asNumber = Number(occurredAtMs);
  const deltaMs = Math.max(0, nowMs - asNumber);
  if (deltaMs < 5_000) {
    return "just now";
  }
  if (deltaMs < 60_000) {
    return `${Math.floor(deltaMs / 1000)}s ago`;
  }
  if (deltaMs < 3_600_000) {
    return `${Math.floor(deltaMs / 60_000)}m ago`;
  }
  if (deltaMs < 86_400_000) {
    return `${Math.floor(deltaMs / 3_600_000)}h ago`;
  }
  return `${Math.floor(deltaMs / 86_400_000)}d ago`;
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
