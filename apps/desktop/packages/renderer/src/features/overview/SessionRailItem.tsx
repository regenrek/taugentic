import type { SessionId, SessionOverview } from "@taugentic/desktop-shared";

import { Badge } from "@/components/ui/badge";
import { ContextMenu } from "@/components/ui/context-menu";
import { DropdownMenu } from "@/components/ui/dropdown-menu";
import { StatusDot, type StatusTone } from "@/components/ui/status-dot";
import { registerLaneRow } from "@/features/agent-visualization/lane-effects";
import { cn } from "@/lib/ui/cn";

import { LaneStatusBadge } from "./LaneStatusBadge";
import { describeLaneStatus, type OverviewLaneTone } from "./formatters";

// Module-level memo for the lane-row callback ref. We deliberately avoid
// `useCallback` / `useRef` here so existing tests that invoke the rail item
// component as a plain function (not via React) keep working. The cache is
// keyed by the canonical session id; each callback registers/unregisters
// against the lane-effects registry and is stable across re-renders.
const laneRowCallbackCache = new Map<string, (node: HTMLDivElement | null) => void>();

function getLaneRowCallback(sessionId: string): (node: HTMLDivElement | null) => void {
  const existing = laneRowCallbackCache.get(sessionId);
  if (existing !== undefined) {
    return existing;
  }
  const callback = (node: HTMLDivElement | null) => {
    registerLaneRow(sessionId, node);
  };
  laneRowCallbackCache.set(sessionId, callback);
  return callback;
}

export interface SessionRailItemProps {
  overview: SessionOverview;
  selected: boolean;
  tabIndex: number;
  onSelect: (sessionId: SessionId) => void;
  /**
   * Optional: cancel the session's active run. Wired by callers that have
   * a cancel mutation. When omitted, the menu item logs a TODO warning so
   * the affordance is still discoverable for the operator.
   */
  onCancelRun?: (sessionId: SessionId) => void;
  /**
   * Optional override for `navigator.clipboard.writeText`, primarily for
   * tests. Defaults to the browser clipboard when omitted.
   */
  copySessionId?: (sessionId: SessionId) => void;
}

export function SessionRailItem({
  copySessionId,
  onCancelRun,
  onSelect,
  overview,
  selected,
  tabIndex,
}: SessionRailItemProps) {
  const presentation = describeLaneStatus(overview.laneStatus);
  const tone = toStatusTone(presentation.tone);
  const preview = overview.lastEventPreview ?? "no recent activity";
  const pending = overview.pendingApprovalCount;

  const bandColor = selected ? "var(--accent)" : `var(--status-${tone})`;
  const rowClass = cn(
    "relative flex w-full cursor-pointer items-start gap-2 border-b border-[var(--border)] pl-3 pr-2 py-2 text-left outline-none transition-colors focus-visible:bg-[var(--bg-raised)] focus-visible:ring-1 focus-visible:ring-[var(--accent)] hover:bg-[var(--surface-overlay)]",
    selected ? "bg-[var(--bg-raised)]" : undefined,
  );

  // Bridge from cortex bus -> lane row DOM. The callback ref is memoized
  // at module scope (keyed by session id) so React keeps a stable identity
  // and registry mutations are idempotent across re-renders.
  const laneRowRef = getLaneRowCallback(overview.session.id);

  function handleFocus() {
    onSelect(overview.session.id);
  }

  function handleCancelRun() {
    if (onCancelRun) {
      onCancelRun(overview.session.id);
      return;
    }
    // TODO(t-lr4l-cortex-bus): wire actual cancel-run mutation when the
    // canonical hook lands. Until then, surface the affordance with a warning.
    console.warn(
      `[SessionRailItem] cancel-run requested for ${overview.session.id} but no cancel hook is wired yet`,
    );
  }

  function handleCopyId() {
    const sessionId = overview.session.id;
    if (copySessionId) {
      copySessionId(sessionId);
      return;
    }
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(sessionId);
    }
  }

  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger
        aria-selected={selected}
        className={rowClass}
        data-session-id={overview.session.id}
        data-lane-status={overview.laneStatus}
        data-selected={selected ? "true" : "false"}
        data-tone={tone}
        onClick={handleFocus}
        ref={laneRowRef}
        role="option"
        tabIndex={tabIndex}
      >
        <span
          aria-hidden="true"
          className="absolute inset-y-0 left-0 w-px"
          style={{ backgroundColor: bandColor }}
        />
        <StatusDot tone={tone} className="mt-[6px] shrink-0" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-[var(--font-mono)] text-[12px] text-[var(--fg)]">
              {overview.session.title}
            </span>
            <LaneStatusBadge
              className="ml-1 shrink-0"
              laneStatus={overview.laneStatus}
              showLabel={false}
            />
            {pending > 0 ? (
              <Badge
                variant="destructive"
                className="ml-auto shrink-0 px-1 py-0 text-[9px] leading-none"
              >
                {pending}
              </Badge>
            ) : null}
          </div>
          <div className="truncate font-[var(--font-mono)] text-[10px] text-[var(--fg-mute)]">
            {overview.session.id}
          </div>
          <div className="overflow-hidden text-ellipsis whitespace-nowrap font-[var(--font-mono)] text-[11px] text-[var(--fg-dim)]">
            {preview}
          </div>
        </div>
        <SessionRailRowMenuTrigger
          onCancelRun={handleCancelRun}
          onCopyId={handleCopyId}
          onFocus={handleFocus}
          sessionTitle={overview.session.title}
        />
      </ContextMenu.Trigger>
      <ContextMenu.Content className="min-w-[12rem]" data-session-rail-context-menu>
        <SessionRailRowMenuItems
          MenuItem={ContextMenu.Item}
          onCancelRun={handleCancelRun}
          onCopyId={handleCopyId}
          onFocus={handleFocus}
        />
      </ContextMenu.Content>
    </ContextMenu.Root>
  );
}

function SessionRailRowMenuTrigger({
  onCancelRun,
  onCopyId,
  onFocus,
  sessionTitle,
}: {
  onCancelRun: () => void;
  onCopyId: () => void;
  onFocus: () => void;
  sessionTitle: string;
}) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger
        aria-label={`Session actions for ${sessionTitle}`}
        className="ml-1 inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-[var(--radius-sm)] border border-transparent text-[var(--fg-mute)] transition-colors hover:border-[var(--border)] hover:bg-[var(--surface-overlay)] hover:text-[var(--fg)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--accent)]"
        data-session-rail-row-menu-trigger
        onClick={(event) => event.stopPropagation()}
        type="button"
      >
        <span aria-hidden="true">⋯</span>
      </DropdownMenu.Trigger>
      <DropdownMenu.Content
        align="end"
        className="min-w-[12rem]"
        data-session-rail-row-menu
        side="bottom"
      >
        <SessionRailRowMenuItems
          MenuItem={DropdownMenu.Item}
          onCancelRun={onCancelRun}
          onCopyId={onCopyId}
          onFocus={onFocus}
        />
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  );
}

function SessionRailRowMenuItems({
  MenuItem,
  onCancelRun,
  onCopyId,
  onFocus,
}: {
  MenuItem: typeof DropdownMenu.Item | typeof ContextMenu.Item;
  onCancelRun: () => void;
  onCopyId: () => void;
  onFocus: () => void;
}) {
  return (
    <>
      <MenuItem data-session-rail-action="focus" onClick={onFocus}>
        Focus this session
      </MenuItem>
      <MenuItem data-session-rail-action="cancel" onClick={onCancelRun}>
        Cancel run
      </MenuItem>
      <MenuItem data-session-rail-action="copy" onClick={onCopyId}>
        Copy session ID
      </MenuItem>
      <MenuItem
        data-session-rail-action="view-raw"
        disabled
        title="available in focused run Raw tab"
      >
        View raw event
      </MenuItem>
    </>
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
