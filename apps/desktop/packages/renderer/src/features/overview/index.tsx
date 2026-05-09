import { useMemo } from "react";

import type { SessionId, SessionOverview } from "@taugentic/desktop-shared";

import { cn } from "@/lib/ui/cn";

import { useSessionOverviewQuery } from "@/lib/queries/session-queries";

import { RailHeaderCountsView } from "./RailHeaderCounts";
import { SessionRailItem } from "./SessionRailItem";
import { aggregateLaneCounts, sortSessionsForOperator } from "./formatters";
import { WorkInbox } from "../work-inbox";

export { LaneStatusBadge } from "./LaneStatusBadge";
export type { LaneStatusBadgeProps } from "./LaneStatusBadge";
export { RailHeaderCountsView } from "./RailHeaderCounts";
export type { RailHeaderCountsViewProps } from "./RailHeaderCounts";
export { SessionRailItem };
export type { SessionRailItemProps } from "./SessionRailItem";

export interface SessionRailProps {
  onSelect: (sessionId: SessionId | null) => void;
  selectedSessionId: SessionId | null;
}

export function SessionRail({ onSelect, selectedSessionId }: SessionRailProps) {
  const overview = useSessionOverviewQuery();

  const sessions = useMemo(
    () => sortSessionsForOperator(overview.data?.sessions ?? []),
    [overview.data],
  );

  const hasLoaded = overview.data !== undefined;
  const isInitialLoading = overview.isLoading;
  const errorMessage = overview.error ? toErrorMessage(overview.error) : null;
  const hasInitialError = !hasLoaded && errorMessage !== null;

  return (
    <SessionRailView
      errorMessage={errorMessage}
      hasLoaded={hasLoaded}
      hasInitialError={hasInitialError}
      isInitialLoading={isInitialLoading}
      onSelect={onSelect}
      selectedSessionId={selectedSessionId}
      sessions={sessions}
    />
  );
}

export interface SessionRailViewProps {
  errorMessage: string | null;
  hasLoaded: boolean;
  hasInitialError: boolean;
  isInitialLoading: boolean;
  onSelect: (sessionId: SessionId | null) => void;
  selectedSessionId: SessionId | null;
  sessions: SessionOverview[];
}

export function SessionRailView({
  errorMessage,
  hasInitialError,
  hasLoaded,
  isInitialLoading,
  onSelect,
  selectedSessionId,
  sessions,
}: SessionRailViewProps) {
  const activeIndex = useMemo(
    () => findSelectedIndex(sessions, selectedSessionId),
    [sessions, selectedSessionId],
  );

  function clampIndex(index: number): number {
    if (sessions.length === 0) {
      return 0;
    }
    if (index < 0) {
      return 0;
    }
    if (index >= sessions.length) {
      return sessions.length - 1;
    }
    return index;
  }

  function moveSelection(nextIndex: number) {
    const clamped = clampIndex(nextIndex);
    const target = sessions[clamped];
    if (target !== undefined) {
      onSelect(target.session.id);
    }
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    if (sessions.length === 0) {
      return;
    }
    switch (event.key) {
      case "ArrowDown": {
        event.preventDefault();
        moveSelection(activeIndex + 1);
        return;
      }
      case "ArrowUp": {
        event.preventDefault();
        moveSelection(activeIndex - 1);
        return;
      }
      case "Home": {
        event.preventDefault();
        moveSelection(0);
        return;
      }
      case "End": {
        event.preventDefault();
        moveSelection(sessions.length - 1);
        return;
      }
      case "Enter":
      case " ": {
        event.preventDefault();
        moveSelection(activeIndex);
        return;
      }
      default:
        return;
    }
  }

  const counts = useMemo(() => aggregateLaneCounts(sessions), [sessions]);

  return (
    <div className="flex h-full w-full flex-col">
      <SessionRailWorkspaceNotice />
      <RailHeaderCountsView counts={counts} />
      <WorkInbox selectedSessionId={selectedSessionId} />
      <SessionRailBody
        activeIndex={activeIndex}
        errorMessage={errorMessage}
        hasInitialError={hasInitialError}
        hasLoaded={hasLoaded}
        isInitialLoading={isInitialLoading}
        onKeyDown={handleKeyDown}
        onSelect={onSelect}
        selectedSessionId={selectedSessionId}
        sessions={sessions}
      />
    </div>
  );
}

function SessionRailBody({
  activeIndex,
  errorMessage,
  hasInitialError,
  hasLoaded,
  isInitialLoading,
  onKeyDown,
  onSelect,
  selectedSessionId,
  sessions,
}: {
  activeIndex: number;
  errorMessage: string | null;
  hasInitialError: boolean;
  hasLoaded: boolean;
  isInitialLoading: boolean;
  onKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
  onSelect: (sessionId: SessionId | null) => void;
  selectedSessionId: SessionId | null;
  sessions: SessionOverview[];
}) {
  if (isInitialLoading) {
    return (
      <div
        aria-label="Sessions"
        aria-busy="true"
        className="flex flex-1 flex-col"
        data-state="loading"
        role="listbox"
      >
        <div className="px-3 py-2 font-[var(--font-mono)] text-[12px] text-[var(--fg-mute)]">
          Loading sessions…
        </div>
      </div>
    );
  }

  if (hasInitialError) {
    return (
      <div aria-label="Sessions" className="flex flex-1 flex-col" data-state="error" role="listbox">
        <div className="px-3 py-2 font-[var(--font-mono)] text-[12px] text-[var(--status-failed)]">
          error: {errorMessage ?? "session overview unavailable"}
        </div>
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div aria-label="Sessions" className="flex flex-1 flex-col" data-state="empty" role="listbox">
        <div className="px-3 py-2 font-[var(--font-mono)] text-[12px] text-[var(--fg-dim)]">
          No sessions yet. Open a trusted workspace from the Sessions panel.
        </div>
      </div>
    );
  }

  return (
    <div
      aria-label="Sessions"
      className={cn("flex flex-1 flex-col outline-none")}
      data-state={hasLoaded ? "ready" : "loading"}
      onKeyDown={onKeyDown}
      role="listbox"
      tabIndex={0}
    >
      {sessions.map((overview, index) => (
        <SessionRailItem
          key={overview.session.id}
          onSelect={onSelect}
          overview={overview}
          selected={selectedSessionId === overview.session.id}
          tabIndex={index === activeIndex ? 0 : -1}
        />
      ))}
      {hasLoaded && errorMessage !== null ? (
        <div
          className="border-t border-[var(--status-failed)]/40 bg-[var(--bg-raised)] px-3 py-1.5 font-[var(--font-mono)] text-[11px] text-[var(--status-failed)]"
          data-state="stale"
        >
          stale · {errorMessage}
        </div>
      ) : null}
    </div>
  );
}

function SessionRailWorkspaceNotice() {
  return (
    <div className="sticky top-0 z-10 border-b border-[var(--border)] bg-[var(--bg-raised)] px-3 py-2 font-[var(--font-mono)] text-[11px] text-[var(--fg-mute)] backdrop-blur-sm">
      Open sessions from the Sessions panel after choosing a trusted workspace.
    </div>
  );
}

function findSelectedIndex(
  sessions: SessionOverview[],
  selectedSessionId: SessionId | null,
): number {
  if (selectedSessionId === null || sessions.length === 0) {
    return 0;
  }
  const index = sessions.findIndex((overview) => overview.session.id === selectedSessionId);
  return index >= 0 ? index : 0;
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
