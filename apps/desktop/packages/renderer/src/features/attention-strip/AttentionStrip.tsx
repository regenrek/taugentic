import { useMemo } from "react";

import type { SessionOverviewResult } from "@taugentic/desktop-shared";

import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { StatusDot, type StatusTone } from "@/components/ui/status-dot";

import type { DaemonControlModel } from "@/features/daemon/model";
import { useSessionOverviewQuery } from "@/lib/queries/session-queries";

export interface AttentionStripState {
  daemonHealthy: boolean;
  daemonLabel: string;
  failuresCount: number;
  pendingApprovalsCount: number;
  /** Error from the last session overview read. Independent of daemon health. */
  feedErrorMessage: string | null;
  /** Whether the session overview read has ever successfully loaded. */
  feedHasLoaded: boolean;
}

export interface AttentionStripProps {
  daemon: DaemonControlModel;
}

export function AttentionStrip({ daemon }: AttentionStripProps) {
  const query = useSessionOverviewQuery();
  const counts = useMemo(
    () =>
      query.data === undefined
        ? { failuresCount: 0, pendingApprovalsCount: 0 }
        : deriveFeedCounts(query.data),
    [query.data],
  );
  const feedHasLoaded = query.data !== undefined;
  // A transient polling failure after first load becomes a visible feed pill;
  // before first load, the feed is simply "not yet loaded".
  const feedErrorMessage = query.error && feedHasLoaded ? toErrorMessage(query.error) : null;
  const { daemonHealthy, daemonLabel } = deriveDaemonHealth(daemon);

  const state: AttentionStripState = {
    daemonHealthy,
    daemonLabel,
    failuresCount: counts.failuresCount,
    pendingApprovalsCount: counts.pendingApprovalsCount,
    feedErrorMessage,
    feedHasLoaded,
  };

  return <AttentionStripView state={state} />;
}

export interface AttentionStripViewProps {
  state: AttentionStripState;
}

export function AttentionStripView({ state }: AttentionStripViewProps) {
  const approvalsTone: StatusTone = state.pendingApprovalsCount > 0 ? "waiting" : "idle";
  const failuresTone: StatusTone = state.failuresCount > 0 ? "failed" : "idle";
  const daemonTone: StatusTone = state.daemonHealthy ? "active" : "failed";
  const feedTone: StatusTone = state.feedErrorMessage === null ? "idle" : "failed";

  return (
    <div
      aria-label="Attention strip"
      className="flex items-center gap-3 bg-[var(--bg)] px-3 py-2 font-[var(--font-mono)] text-[var(--fg)]"
      data-feature="attention-strip"
    >
      <AttentionPill count={state.pendingApprovalsCount} label="APPROVALS" tone={approvalsTone} />
      <Separator orientation="vertical" className="h-4" />
      <AttentionPill count={state.failuresCount} label="FAILURES" tone={failuresTone} />
      <Separator orientation="vertical" className="h-4" />
      <AttentionDaemonPill label={state.daemonLabel} tone={daemonTone} />
      {state.feedErrorMessage !== null ? (
        <>
          <Separator orientation="vertical" className="h-4" />
          <AttentionFeedPill message={state.feedErrorMessage} tone={feedTone} />
        </>
      ) : null}
    </div>
  );
}

function AttentionPill({ count, label, tone }: { count: number; label: string; tone: StatusTone }) {
  return (
    <Badge
      className="gap-2 border-transparent bg-transparent px-0 py-0 text-[11px] tracking-[0.18em]"
      data-attention-pill={label.toLowerCase()}
      data-tone={tone}
      variant="secondary"
    >
      <StatusDot tone={tone} />
      <span className="text-[var(--fg-dim)]">{label}</span>
      <span className="text-[var(--fg-mute)]">·</span>
      <span className="tabular-nums text-[var(--fg)]">{count}</span>
    </Badge>
  );
}

function AttentionDaemonPill({ label, tone }: { label: string; tone: StatusTone }) {
  return (
    <Badge
      className="gap-2 border-transparent bg-transparent px-0 py-0 text-[11px] tracking-[0.18em]"
      data-attention-pill="daemon"
      data-tone={tone}
      variant="secondary"
    >
      <StatusDot tone={tone} />
      <span className="text-[var(--fg-dim)]">DAEMON</span>
      <span className="text-[var(--fg-mute)]">·</span>
      <span className="text-[var(--fg)]">{label}</span>
    </Badge>
  );
}

function AttentionFeedPill({ message, tone }: { message: string; tone: StatusTone }) {
  return (
    <Badge
      className="gap-2 border-transparent bg-transparent px-0 py-0 text-[11px] tracking-[0.18em]"
      data-attention-pill="feed"
      data-tone={tone}
      variant="secondary"
    >
      <StatusDot tone={tone} />
      <span className="text-[var(--fg-dim)]">FEED</span>
      <span className="text-[var(--fg-mute)]">·</span>
      <span className="max-w-[16rem] truncate text-[var(--status-failed)]" title={message}>
        {message}
      </span>
    </Badge>
  );
}

function deriveFeedCounts(result: SessionOverviewResult): {
  failuresCount: number;
  pendingApprovalsCount: number;
} {
  let pendingApprovalsCount = 0;
  let failuresCount = 0;
  for (const session of result.sessions ?? []) {
    pendingApprovalsCount += session.pendingApprovalCount;
    if (session.laneStatus === "failed") {
      failuresCount += 1;
    }
  }
  return { failuresCount, pendingApprovalsCount };
}

function deriveDaemonHealth(model: DaemonControlModel): {
  daemonHealthy: boolean;
  daemonLabel: string;
} {
  if (model.state === null) {
    return {
      daemonHealthy: false,
      daemonLabel: model.errorMessage === null ? "loading" : "unavailable",
    };
  }
  const { actualMode, transitionStatus } = model.state;
  const daemonHealthy =
    (actualMode === "local" || actualMode === "background") && model.errorMessage === null;
  return {
    daemonHealthy,
    daemonLabel: `${actualMode} · ${transitionStatus}`,
  };
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
