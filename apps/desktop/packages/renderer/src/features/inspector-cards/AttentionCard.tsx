import { useMemo } from "react";

import type { SessionId, SessionOverview, SessionOverviewResult } from "@taugentic/desktop-shared";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusDot } from "@/components/ui/status-dot";
import { persistCurrentSessionId } from "@/features/sessions/selection";
import { workspaceShellStore } from "@/features/workspace/state/store";
import { useSessionOverviewQuery } from "@/lib/queries/session-queries";

export const ATTENTION_CARD_MAX_ROWS = 5;

/**
 * Right-rail inspector card: pending approvals across all sessions.
 *
 * Reads from the existing session-overview query (no new daemon owner).
 * Focuses sessions through the existing workspace shell store / persistence
 * helpers so it matches the SessionRail focus path exactly.
 */
export function AttentionCard() {
  const overview = useSessionOverviewQuery();
  const items = useMemo(() => deriveAttentionItems(overview.data), [overview.data]);
  const totalPending = useMemo(() => totalPendingApprovals(overview.data), [overview.data]);

  return (
    <AttentionCardView
      errorMessage={overview.error ? toErrorMessage(overview.error) : null}
      hasLoaded={overview.data !== undefined}
      isLoading={overview.isLoading}
      items={items}
      onFocusSession={focusSession}
      totalPending={totalPending}
    />
  );
}

export interface AttentionCardItem {
  laneStatus: SessionOverview["laneStatus"];
  pendingApprovalCount: number;
  preview: string;
  sessionId: SessionId;
  sessionTitle: string;
}

export interface AttentionCardViewProps {
  errorMessage: string | null;
  hasLoaded: boolean;
  isLoading: boolean;
  items: AttentionCardItem[];
  onFocusSession: (sessionId: SessionId) => void;
  totalPending: number;
}

export function AttentionCardView({
  errorMessage,
  hasLoaded,
  isLoading,
  items,
  onFocusSession,
  totalPending,
}: AttentionCardViewProps) {
  const headerCount = totalPending;

  return (
    <Card
      aria-label="Attention"
      className="border-x-0 border-t-0 rounded-none"
      data-feature="attention-card"
    >
      <CardHeader className="flex flex-row items-center justify-between px-3 pt-3 pb-2">
        <CardTitle className="text-[10px] uppercase tracking-[0.18em] text-[var(--fg-mute)]">
          ATTENTION
        </CardTitle>
        <span
          className="font-[var(--font-mono)] text-[11px] tabular-nums text-[var(--fg-dim)]"
          data-attention-total
        >
          {headerCount} pending
        </span>
      </CardHeader>
      <CardContent className="px-3 pb-3">
        {isLoading && !hasLoaded ? (
          <div className="text-[12px] text-[var(--fg-dim)]" data-state="loading">
            Loading approvals…
          </div>
        ) : errorMessage !== null && !hasLoaded ? (
          <div className="text-[12px] text-[var(--status-failed)]" data-state="error">
            error: {errorMessage}
          </div>
        ) : items.length === 0 ? (
          <div className="text-[12px] text-[var(--fg-dim)]" data-state="empty">
            no pending approvals
          </div>
        ) : (
          <ul className="flex flex-col gap-1.5" data-state="ready">
            {items.map((item) =>
              AttentionRow({ item, key: item.sessionId, onFocus: onFocusSession }),
            )}
          </ul>
        )}
        {errorMessage !== null && hasLoaded ? (
          <div className="mt-2 text-[11px] text-[var(--status-failed)]" data-state="stale">
            stale · {errorMessage}
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

export function AttentionRow({
  item,
  key,
  onFocus,
}: {
  item: AttentionCardItem;
  key?: string;
  onFocus: (sessionId: SessionId) => void;
}) {
  return (
    <li
      className="flex items-center justify-between gap-2 border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-1.5 font-[var(--font-mono)] text-[12px] text-[var(--fg)]"
      data-attention-row
      data-session-id={item.sessionId}
      key={key}
      title={`${item.sessionTitle} · ${item.pendingApprovalCount} pending · ${item.laneStatus}`}
    >
      <div className="flex min-w-0 items-center gap-2">
        <StatusDot tone="waiting" />
        <div className="min-w-0">
          <div className="truncate text-[var(--fg)]">{item.sessionTitle}</div>
          <div className="truncate text-[10px] text-[var(--fg-mute)]">{item.preview}</div>
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <span
          className="rounded-[var(--radius-sm)] bg-[var(--status-waiting)]/15 px-1.5 py-0.5 text-[10px] font-medium tabular-nums text-[var(--status-waiting)]"
          data-attention-pending-count
        >
          {item.pendingApprovalCount}
        </span>
        <Button
          aria-label={`Focus session ${item.sessionTitle}`}
          className="h-6 px-2 text-[10px] uppercase tracking-[0.16em]"
          data-attention-focus
          onClick={() => onFocus(item.sessionId)}
          size="sm"
          type="button"
          variant="ghost"
        >
          Focus
        </Button>
      </div>
    </li>
  );
}

export function deriveAttentionItems(
  result: SessionOverviewResult | undefined,
): AttentionCardItem[] {
  if (result === undefined) {
    return [];
  }
  return (result.sessions ?? [])
    .filter((session) => session.pendingApprovalCount > 0)
    .sort((left, right) => {
      if (left.pendingApprovalCount !== right.pendingApprovalCount) {
        return right.pendingApprovalCount - left.pendingApprovalCount;
      }
      const leftActivity = left.lastActivityAtMs ?? 0n;
      const rightActivity = right.lastActivityAtMs ?? 0n;
      if (leftActivity === rightActivity) {
        return left.session.id.localeCompare(right.session.id);
      }
      return rightActivity > leftActivity ? 1 : -1;
    })
    .slice(0, ATTENTION_CARD_MAX_ROWS)
    .map((session) => ({
      laneStatus: session.laneStatus,
      pendingApprovalCount: session.pendingApprovalCount,
      preview: session.lastEventPreview ?? "no recent activity",
      sessionId: session.session.id,
      sessionTitle: session.session.title,
    }));
}

export function totalPendingApprovals(result: SessionOverviewResult | undefined): number {
  if (result === undefined) {
    return 0;
  }
  let total = 0;
  for (const session of result.sessions ?? []) {
    total += session.pendingApprovalCount;
  }
  return total;
}

function focusSession(sessionId: SessionId): void {
  workspaceShellStore.trigger.sessionChanged({ sessionId });
  persistCurrentSessionId(sessionId);
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
