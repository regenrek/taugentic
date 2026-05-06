/*
 * AgentRunStream.
 *
 * Live tail of recent activity items for the focused session, painted as
 * a phosphor-decay list. Latest events arrive at the bottom and briefly
 * glow before settling to dim foreground.
 *
 * Renders every Harness-v1 event variant as first-class terminal lines,
 * including `agentStream` frames (assistantTurnStarted / MessageDelta /
 * TurnCompleted, toolCallStarted / Progressed / Completed,
 * pendingStateChanged). Ordering is by `cursor.sequence` (daemon-assigned),
 * not wall-clock timestamps; tool calls correlate through
 * `AgentStreamEvent.itemId` carried in the formatted line.
 *
 * Auto-follow model:
 *  - A bottom-anchor element is re-keyed whenever the latest line changes,
 *    so the ref callback re-fires on every new line. When the user is in
 *    follow mode the anchor scrolls itself into view; when the user has
 *    scrolled up, the anchor is inert and the view stays put.
 *  - A `Jump to latest` affordance restores follow mode on demand.
 *
 * The stream subscribes through the canonical TanStack Query hook
 * (`useSessionActivityQuery`) -- no new daemon-derived domain owner is
 * introduced. We intentionally cap at <=200 entries so the panel
 * stays cheap regardless of session length.
 */

import { useCallback, useRef, useState } from "react";

import type { SessionId } from "@taugentic/desktop-shared";

import { phosphorDecayClass } from "@/features/cortex-canvas";
import { useSessionActivityQuery } from "@/lib/queries/session-queries";

import { formatActivityLine, sortActivityAscending } from "@/features/session-detail/formatters";
import { pickRecentActivity } from "./selectors/focused-run";

const MAX_TAIL_ENTRIES = 200;
const NEAR_BOTTOM_PX = 32;

export interface AgentRunStreamProps {
  sessionId: SessionId;
}

/**
 * Pure helper: decide whether the viewport should stay pinned to the live
 * tail given the current scroll metrics. Exported for unit tests.
 */
export function computeFollowLiveTail(args: {
  readonly scrollTop: number;
  readonly scrollHeight: number;
  readonly clientHeight: number;
  readonly nearBottomPx?: number;
}): boolean {
  const threshold = args.nearBottomPx ?? NEAR_BOTTOM_PX;
  const distanceFromBottom = args.scrollHeight - (args.scrollTop + args.clientHeight);
  return distanceFromBottom <= threshold;
}

function AgentRunStream({ sessionId }: AgentRunStreamProps) {
  const query = useSessionActivityQuery(sessionId);
  const recent = pickRecentActivity(query.data, MAX_TAIL_ENTRIES);
  const ascending = sortActivityAscending(recent);
  const lines = ascending.map(formatActivityLine);
  const lastLineKey = lines.length === 0 ? "__empty__" : lines[lines.length - 1]!.key;

  const containerRef = useRef<HTMLDivElement | null>(null);
  const followLiveTailRef = useRef(true);
  const [isFollowing, setIsFollowing] = useState(true);

  const setFollow = useCallback((follow: boolean) => {
    if (followLiveTailRef.current !== follow) {
      followLiveTailRef.current = follow;
      setIsFollowing(follow);
    }
  }, []);

  const containerRefCallback = useCallback((node: HTMLDivElement | null) => {
    containerRef.current = node;
  }, []);

  // Bottom anchor is re-keyed by the latest line key so this callback
  // re-fires on every new line. That's what actually makes auto-follow
  // track the live tail (the earlier mount-only ref callback never ran
  // for appended content).
  const bottomAnchorRefCallback = useCallback((node: HTMLDivElement | null) => {
    if (node === null) {
      return;
    }
    if (followLiveTailRef.current) {
      node.scrollIntoView({ block: "end", inline: "nearest" });
    }
  }, []);

  function handleScroll(event: React.UIEvent<HTMLDivElement>): void {
    const el = event.currentTarget;
    setFollow(
      computeFollowLiveTail({
        scrollTop: el.scrollTop,
        scrollHeight: el.scrollHeight,
        clientHeight: el.clientHeight,
      }),
    );
  }

  function handleJumpToLatest(): void {
    setFollow(true);
    const el = containerRef.current;
    if (el !== null) {
      el.scrollTop = el.scrollHeight;
    }
  }

  if (lines.length === 0) {
    return (
      <div
        aria-label="Agent run stream"
        className="flex max-h-[180px] min-h-[88px] items-center justify-center border-t border-b border-[var(--border)] bg-[var(--bg-sunken)] px-3 py-3 font-[var(--font-mono)] text-[11px] text-[var(--fg-mute)]"
        data-agent-visualization-stream="empty"
        role="log"
      >
        no activity yet for this session
      </div>
    );
  }

  return (
    <div className="relative" data-agent-visualization-stream-wrapper="">
      <div
        aria-label="Agent run stream"
        className="max-h-[180px] min-h-[88px] overflow-y-auto border-t border-b border-[var(--border)] bg-[var(--bg-sunken)] px-3 py-2 font-[var(--font-mono)] text-[11px] leading-5 text-[var(--fg-dim)]"
        data-agent-visualization-stream="ready"
        data-agent-visualization-follow={isFollowing ? "true" : "false"}
        data-stream-lines={lines.length}
        onScroll={handleScroll}
        ref={containerRefCallback}
        role="log"
      >
        {lines.map((line) => (
          <div
            className={`whitespace-pre-wrap ${phosphorDecayClass()}`}
            data-line-kind={line.kind}
            key={line.key}
          >
            {line.text}
          </div>
        ))}
        <div
          aria-hidden="true"
          data-agent-visualization-stream-anchor=""
          key={lastLineKey}
          ref={bottomAnchorRefCallback}
        />
      </div>
      {isFollowing ? null : (
        <button
          className="absolute right-2 bottom-2 rounded border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-0.5 font-[var(--font-mono)] text-[10px] uppercase tracking-[0.16em] text-[var(--fg-dim)] shadow-sm hover:text-[var(--fg)]"
          data-agent-visualization-jump-to-latest=""
          onClick={handleJumpToLatest}
          type="button"
        >
          jump to latest
        </button>
      )}
    </div>
  );
}

void AgentRunStream;

export const AGENT_RUN_STREAM_NEAR_BOTTOM_PX = NEAR_BOTTOM_PX;
