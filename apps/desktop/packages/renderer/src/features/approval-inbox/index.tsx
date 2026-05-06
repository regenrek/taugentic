import { useState } from "react";

import type {
  ApprovalDecision,
  ApprovalId,
  ApprovalRequest,
  SessionId,
} from "@taugentic/desktop-shared";

import { Button } from "@/components/ui/button";
import { useDecideApprovalMutation } from "@/lib/queries/session-mutations";
import { useSessionApprovalsQuery } from "@/lib/queries/session-queries";

export interface ApprovalInboxProps {
  nowMs?: number;
  sessionId: SessionId;
}

export function ApprovalInbox({ nowMs = Date.now(), sessionId }: ApprovalInboxProps) {
  const approvals = useSessionApprovalsQuery(sessionId);
  const decideMutation = useDecideApprovalMutation(sessionId);
  const items = approvals.data ?? [];
  const [pendingApprovalId, setPendingApprovalId] = useState<ApprovalId | null>(null);
  const [pendingDecision, setPendingDecision] = useState<ApprovalDecision | null>(null);
  const [commandError, setCommandError] = useState<string | null>(null);
  const errorMessage = approvals.error ? toErrorMessage(approvals.error) : commandError;

  async function decide(approvalId: ApprovalId, decision: ApprovalDecision) {
    setPendingApprovalId(approvalId);
    setPendingDecision(decision);
    setCommandError(null);
    try {
      await decideMutation.mutateAsync({ approvalId, decision });
    } catch (error) {
      setCommandError(toErrorMessage(error));
    } finally {
      setPendingApprovalId((current) => (current === approvalId ? null : current));
      setPendingDecision((current) => (current === decision ? null : current));
    }
  }

  return (
    <section
      aria-label="Approval inbox"
      className="flex flex-col gap-3 px-3 py-3"
      data-section="approval-inbox"
    >
      <div className="flex items-center gap-2">
        <h2 className="text-[11px] font-semibold uppercase tracking-[0.18em] text-[var(--fg-dim)]">
          approval inbox
        </h2>
        <span className="rounded border border-[var(--border)] px-1.5 py-[1px] font-[var(--font-mono)] text-[10px] text-[var(--fg-mute)]">
          {items.length}
        </span>
        {approvals.isFetching ? (
          <span className="ml-auto text-[10px] uppercase tracking-[0.16em] text-[var(--fg-mute)]">
            syncing
          </span>
        ) : null}
      </div>
      {approvals.isLoading ? (
        <p className="text-[12px] text-[var(--fg-mute)]" role="status">
          Loading pending approvals...
        </p>
      ) : null}
      {errorMessage ? (
        <p className="text-[12px] text-[var(--destructive-foreground,#ff6b6b)]" role="alert">
          {errorMessage}
        </p>
      ) : null}
      {!approvals.isLoading && items.length === 0 && !errorMessage ? (
        <p className="text-[12px] text-[var(--fg-mute)]" role="status">
          No pending approvals.
        </p>
      ) : null}
      {items.length > 0 ? (
        <div className="flex flex-col gap-2">
          {items.map((approval) => (
            <ApprovalInboxRow
              key={approval.id}
              approval={approval}
              disabled={pendingApprovalId !== null}
              nowMs={nowMs}
              pendingDecision={pendingApprovalId === approval.id ? pendingDecision : null}
              onDecide={decide}
            />
          ))}
        </div>
      ) : null}
    </section>
  );
}

function ApprovalInboxRow({
  approval,
  disabled,
  nowMs,
  onDecide,
  pendingDecision,
}: {
  approval: ApprovalRequest;
  disabled: boolean;
  nowMs: number;
  onDecide: (approvalId: ApprovalId, decision: ApprovalDecision) => void;
  pendingDecision: ApprovalDecision | null;
}) {
  return (
    <article
      className="flex flex-col gap-2 rounded border border-[var(--border)] bg-[var(--panel)]/40 p-3"
      data-approval-id={approval.id}
      data-approval-target={approval.target.kind}
    >
      <div className="flex items-center gap-2 font-[var(--font-mono)] text-[11px]">
        <span className="rounded border border-[var(--border)] px-1.5 py-[1px] uppercase tracking-[0.16em] text-[10px] text-[var(--fg-dim)]">
          {approval.scope}
        </span>
        <span className="truncate text-[var(--fg-mute)]">run {approval.runId}</span>
        <span className="ml-auto text-[var(--fg-mute)]">{formatTtl(approval, nowMs)}</span>
      </div>
      <div className="text-[13px] text-[var(--fg)]">{describeApprovalTarget(approval.target)}</div>
      <p className="truncate text-[12px] text-[var(--fg-dim)]">
        {approval.reason || "(no reason)"}
      </p>
      <div className="flex items-center gap-2">
        <Button
          disabled={disabled}
          onClick={() => onDecide(approval.id, "approved")}
          size="sm"
          type="button"
          variant="secondary"
        >
          {pendingDecision === "approved" ? "approving..." : "approve"}
        </Button>
        <Button
          disabled={disabled}
          onClick={() => onDecide(approval.id, "rejected")}
          size="sm"
          type="button"
          variant="destructive"
        >
          {pendingDecision === "rejected" ? "denying..." : "deny"}
        </Button>
      </div>
    </article>
  );
}

function describeApprovalTarget(target: ApprovalRequest["target"]): string {
  switch (target.kind) {
    case "toolCall":
      return `Tool call: ${target.toolName}`;
    case "fileWrite":
      return target.paths.length === 0 ? "File write" : `File write: ${target.paths.join(", ")}`;
    case "processExec":
      return target.command ? `Process: ${target.command}` : "Process execution";
    case "networkAccess": {
      const host = target.host ?? "unknown host";
      const protocol = target.protocol ?? "unknown protocol";
      return `Network: ${protocol}://${host}`;
    }
    case "capsuleDispatch": {
      const child = target.childRunId ?? "pending child";
      const scope = target.workspaceScope ?? "default workspace";
      return `Capsule dispatch: ${child} (${scope})`;
    }
  }
}

function formatTtl(approval: Pick<ApprovalRequest, "expiresAtMs">, nowMs: number): string {
  const expiresAtMs = Number(approval.expiresAtMs);
  const remainingMs = expiresAtMs - nowMs;
  if (remainingMs <= 0) {
    return "expired";
  }
  const remainingSeconds = Math.ceil(remainingMs / 1000);
  if (remainingSeconds < 60) {
    return `expires in ${remainingSeconds}s`;
  }
  return `expires in ${Math.ceil(remainingSeconds / 60)}m`;
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
