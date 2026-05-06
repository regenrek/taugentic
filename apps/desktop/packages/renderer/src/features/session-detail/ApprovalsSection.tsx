import { useState, type ReactElement } from "react";

import type {
  ApprovalDecision,
  ApprovalId,
  ApprovalRequest,
  SessionId,
} from "@taugentic/desktop-shared";

import { Button } from "@/components/ui/button";

import { useSessionApprovalsQuery } from "@/lib/queries/session-queries";
import { useDecideApprovalMutation } from "@/lib/queries/session-mutations";

import { describeApprovalReason } from "./formatters";
import { SectionFeedback } from "./section-feedback";
import { SectionHeader } from "./section-header";
import {
  useSessionApprovalLiveSync,
  type SessionApprovalLiveSyncView,
} from "./useSessionApprovalLiveSync";

export interface ApprovalsSectionProps {
  hideWhenEmpty?: boolean;
  sessionId: SessionId;
  variant?: "default" | "mission-control";
}

export function ApprovalsSection({
  hideWhenEmpty = false,
  sessionId,
  variant = "default",
}: ApprovalsSectionProps) {
  const query = useSessionApprovalsQuery(sessionId);
  const decideMutation = useDecideApprovalMutation(sessionId);
  const live = useSessionApprovalLiveSync(sessionId);

  const items = query.data ?? [];
  const hasLoaded = query.data !== undefined;
  const queryError = query.error ? toErrorMessage(query.error) : null;

  const [commandErrorMessage, setCommandErrorMessage] = useState<string | null>(null);
  const [pendingApprovalId, setPendingApprovalId] = useState<ApprovalId | null>(null);
  const [pendingDecision, setPendingDecision] = useState<ApprovalDecision | null>(null);

  const combinedError = queryError ?? commandErrorMessage;
  const isMissionControl = variant === "mission-control";
  const label = isMissionControl ? "approval required" : "approvals";
  const streamStatusLabel = describeStreamStatus(live);
  const anyResolveInFlight = pendingApprovalId !== null;
  const sectionClassName = isMissionControl
    ? "flex flex-col gap-2 border-b border-[var(--status-waiting)]/40 bg-[var(--status-waiting)]/8 px-3 py-3"
    : "flex flex-col gap-2 px-3 py-3";

  async function decide(approvalId: ApprovalId, decision: ApprovalDecision) {
    setPendingApprovalId(approvalId);
    setPendingDecision(decision);
    setCommandErrorMessage(null);
    try {
      await decideMutation.mutateAsync({ approvalId, decision });
    } catch (error) {
      setCommandErrorMessage(toErrorMessage(error));
    } finally {
      setPendingApprovalId((current) => (current === approvalId ? null : current));
      setPendingDecision((current) => (current === decision ? null : current));
    }
  }

  if (hideWhenEmpty && items.length === 0 && combinedError === null) {
    return null;
  }

  return (
    <section className={sectionClassName} data-approval-surface={variant} data-section="approvals">
      <SectionHeader
        count={items.length}
        errorMessage={combinedError}
        hasLoaded={hasLoaded}
        label={label}
        pending={query.isFetching}
        trailing={<StreamStatusIndicator view={live} label={streamStatusLabel} />}
      />
      <SectionFeedback
        errorMessage={combinedError}
        hasLoaded={hasLoaded}
        isEmpty={items.length === 0}
        itemsLabel="pending approvals"
      />
      {items.length > 0 ? (
        <div className="flex flex-col gap-2">
          {items.map((approval) => (
            <ApprovalRow
              key={approval.id}
              approval={approval}
              isPending={pendingApprovalId === approval.id}
              otherPending={anyResolveInFlight && pendingApprovalId !== approval.id}
              pendingDecision={pendingDecision}
              onDecide={decide}
            />
          ))}
        </div>
      ) : null}
    </section>
  );
}

function ApprovalRow({
  approval,
  isPending,
  otherPending,
  onDecide,
  pendingDecision,
}: {
  approval: ApprovalRequest;
  isPending: boolean;
  otherPending: boolean;
  onDecide: (approvalId: ApprovalRequest["id"], decision: "approved" | "rejected") => void;
  pendingDecision: "approved" | "rejected" | null;
}) {
  const disabled = isPending || otherPending;
  const toolCallId = approval.toolCallId ?? null;

  return (
    <div
      className="flex flex-col gap-1 border-t border-[var(--border)] pt-2 font-[var(--font-mono)] text-[12px] text-[var(--fg)]"
      data-approval-id={approval.id}
      data-approval-scope={approval.scope}
      data-approval-pending={isPending ? "true" : undefined}
    >
      <div className="flex items-center gap-2">
        <span
          className="rounded border border-[var(--border)] px-1.5 py-[1px] uppercase tracking-[0.16em] text-[10px] text-[var(--fg-dim)]"
          data-approval-scope-badge=""
        >
          {approval.scope}
        </span>
        <span className="truncate text-[11px] text-[var(--fg-mute)]">run {approval.runId}</span>
        {toolCallId !== null ? (
          <span
            className="truncate text-[11px] text-[var(--fg-mute)]"
            data-approval-tool-call-id={toolCallId}
            title={`tool call ${toolCallId}`}
          >
            · tool {toolCallId}
          </span>
        ) : null}
        <span className="ml-auto truncate text-[10px] text-[var(--fg-mute)]" title={approval.id}>
          {approval.id}
        </span>
      </div>
      <div className="truncate text-[var(--fg-dim)]">{describeApprovalReason(approval)}</div>
      <div className="flex items-center gap-2 pt-1">
        <Button
          disabled={disabled}
          onClick={() => onDecide(approval.id, "approved")}
          size="sm"
          type="button"
          variant="secondary"
        >
          {isPending && pendingDecision === "approved" ? "approving…" : "approve"}
        </Button>
        <Button
          disabled={disabled}
          onClick={() => onDecide(approval.id, "rejected")}
          size="sm"
          type="button"
          variant="destructive"
        >
          {isPending && pendingDecision === "rejected" ? "rejecting…" : "reject"}
        </Button>
      </div>
    </div>
  );
}

function StreamStatusIndicator({
  label,
  view,
}: {
  label: string;
  view: SessionApprovalLiveSyncView;
}): ReactElement {
  return (
    <span
      className={[
        "inline-flex items-center gap-1 text-[10px] uppercase tracking-[0.18em]",
        streamStatusToneClass(view.streamStatus),
      ].join(" ")}
      data-approval-stream-indicator={view.streamStatus}
      title={
        view.streamStatus === "error" && view.errorMessage !== null ? view.errorMessage : label
      }
    >
      <span
        aria-hidden="true"
        className={[
          "inline-block h-1.5 w-1.5 rounded-full",
          streamStatusDotClass(view.streamStatus),
        ].join(" ")}
      />
      live: {label}
    </span>
  );
}

function streamStatusToneClass(status: SessionApprovalLiveSyncView["streamStatus"]): string {
  switch (status) {
    case "ready":
      return "text-[var(--fg-dim)]";
    case "connecting":
      return "text-[var(--fg-mute)]";
    case "error":
      return "text-[var(--destructive-foreground,#ff6b6b)]";
  }
}

function streamStatusDotClass(status: SessionApprovalLiveSyncView["streamStatus"]): string {
  switch (status) {
    case "ready":
      return "bg-emerald-500";
    case "connecting":
      return "bg-amber-500";
    case "error":
      return "bg-rose-500";
  }
}

function describeStreamStatus(view: SessionApprovalLiveSyncView): string {
  switch (view.streamStatus) {
    case "ready":
      return view.lastSequence === null ? "ready" : `ready #${view.lastSequence.toString()}`;
    case "connecting":
      return "connecting";
    case "error":
      return "error";
  }
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
