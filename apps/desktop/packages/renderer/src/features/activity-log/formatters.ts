import type {
  PublicDaemonEvent,
  PublicDaemonEventEnvelope,
  SessionOverviewResult,
} from "@taugentic/desktop-shared";

export type ActivityEventKind =
  | "run"
  | "approval"
  | "artifact"
  | "context_receipt"
  | "agent_stream"
  | "run_reconciled"
  | "token_usage"
  | "conflict"
  | "budget"
  | "session";

export function eventKind(envelope: PublicDaemonEventEnvelope): ActivityEventKind {
  const event = envelope.event as PublicDaemonEvent;
  if ("run" in event) {
    return "run";
  }
  if ("approval" in event) {
    return "approval";
  }
  if ("artifact" in event) {
    return "artifact";
  }
  if ("contextReceipt" in event) {
    return "context_receipt";
  }
  if ("agentStream" in event) {
    return "agent_stream";
  }
  if ("runReconciledOnStartup" in event) {
    return "run_reconciled";
  }
  if ("tokenUsageRecorded" in event) {
    return "token_usage";
  }
  if ("conflict" in event) {
    return "conflict";
  }
  if ("budget" in event) {
    return "budget";
  }
  return "session";
}

export function formatEventSummary(envelope: PublicDaemonEventEnvelope): string {
  const event = envelope.event as PublicDaemonEvent;
  if ("run" in event) {
    const { runId, status, detail } = event.run;
    const trimmed = detail.trim();
    return trimmed.length === 0 ? `run ${runId} ${status}` : `run ${runId} ${status} — ${trimmed}`;
  }
  if ("approval" in event) {
    const approval = event.approval;
    if (approval.phase === "requested") {
      const reason = approval.request.reason.trim();
      const scope = approval.request.scope;
      return reason.length === 0
        ? `approval requested (${scope})`
        : `approval requested (${scope}) — ${reason}`;
    }
    const { approvalId, decision } = approval.resolution;
    return `approval ${approvalId} ${decision}`;
  }
  if ("artifact" in event) {
    const { id, kind, storagePath } = event.artifact.artifact;
    return `artifact ${id} (${kind}) @ ${storagePath}`;
  }
  if ("contextReceipt" in event) {
    const { phase, receipt } = event.contextReceipt;
    return `context receipt ${receipt.id} ${phase} (${receipt.kind}, ${receipt.state})`;
  }
  if ("agentStream" in event) {
    const { runId, frame, itemId, turnId } = event.agentStream;
    const lineage = [turnId, itemId].filter(Boolean).join("/");
    const scope = lineage.length === 0 ? runId : `${runId} ${lineage}`;
    switch (frame.kind) {
      case "assistantTurnStarted":
        return `agent stream ${scope} turn started`;
      case "assistantMessageDelta":
        return `agent stream ${scope} delta`;
      case "assistantTurnCompleted":
        return `agent stream ${scope} turn completed`;
      case "toolCallStarted":
        return `agent stream ${scope} tool ${frame.toolName} started`;
      case "toolCallProgressed":
        return `agent stream ${scope} tool progress`;
      case "toolCallCompleted":
        return `agent stream ${scope} tool ${frame.outcome}`;
      case "pendingStateChanged":
        return `agent stream ${scope} pending ${frame.state}`;
      case "tokenUsageUpdated":
        return `agent stream ${scope} tokens total=${frame.totalTokens ?? "unknown"}`;
    }
  }
  if ("runReconciledOnStartup" in event) {
    const { runId, reason } = event.runReconciledOnStartup;
    return `run ${runId} reconciled on startup — ${reason}`;
  }
  if ("tokenUsageRecorded" in event) {
    const usage = event.tokenUsageRecorded;
    return `token usage ${usage.runId} prompt=${usage.promptTokens} completion=${usage.completionTokens}`;
  }
  if ("conflict" in event) {
    const { run_id: runId, warning } = event.conflict;
    return `conflict warning ${runId} (${warning.conflicts.length} file claim overlap)`;
  }
  if ("budget" in event) {
    const budgetEvent = event.budget.event;
    return `budget exceeded ${budgetEvent.runId} (${budgetEvent.breach.metric}, ${budgetEvent.breach.scope})`;
  }
  const { sessionId, status } = event.session;
  return `session ${sessionId} ${status}`;
}

function envelopeDedupeKey(envelope: PublicDaemonEventEnvelope): string {
  return `${envelope.daemonInstanceId}|${envelope.sessionId}|${envelope.sequence.toString()}`;
}

function compareOccurredAtDesc(
  left: PublicDaemonEventEnvelope,
  right: PublicDaemonEventEnvelope,
): number {
  if (left.occurredAtMs === right.occurredAtMs) {
    if (left.sequence === right.sequence) {
      return 0;
    }
    return right.sequence > left.sequence ? 1 : -1;
  }
  return right.occurredAtMs > left.occurredAtMs ? 1 : -1;
}

export function mergeRecentActivity(result: SessionOverviewResult): PublicDaemonEventEnvelope[] {
  const flat: PublicDaemonEventEnvelope[] = [];
  for (const session of result.sessions ?? []) {
    for (const envelope of session.recentActivity ?? []) {
      flat.push(envelope);
    }
  }
  return dedupeAndSortDesc(flat);
}

export function boundedMerge(
  existing: PublicDaemonEventEnvelope[],
  next: PublicDaemonEventEnvelope[],
  max: number,
): PublicDaemonEventEnvelope[] {
  const combined = existing.concat(next);
  const sorted = dedupeAndSortDesc(combined);
  if (sorted.length <= max) {
    return sorted;
  }
  return sorted.slice(0, max);
}

function dedupeAndSortDesc(envelopes: PublicDaemonEventEnvelope[]): PublicDaemonEventEnvelope[] {
  const seen = new Map<string, PublicDaemonEventEnvelope>();
  for (const envelope of envelopes) {
    const key = envelopeDedupeKey(envelope);
    if (!seen.has(key)) {
      seen.set(key, envelope);
    }
  }
  const deduped = Array.from(seen.values());
  deduped.sort(compareOccurredAtDesc);
  return deduped;
}
