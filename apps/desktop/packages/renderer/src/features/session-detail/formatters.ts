import type {
  AgentTurnRow,
  ActivityPageItem,
  AgentStreamEvent,
  AgentStreamFrame,
  AgentStreamItemId,
  AgentToolCallOutcome,
  ApprovalRequest,
  RunStatus,
  RunSummary,
  RuntimeLanePendingState,
} from "@taugentic/desktop-shared";

import type { LiveAgentMessage, LiveAgentToolCall } from "@/features/agent-stream";
import { assistantLogicalKey, toolLogicalKey } from "@/features/agent-stream";

export type ActivityLineKind = "user" | "agent" | "tool_call" | "tool_result";

export interface ActivityLine {
  kind: ActivityLineKind;
  key: string;
  occurredAtMs: number;
  text: string;
}

/**
 * Canonical mapping from daemon-owned public events to the terminal
 * conversation line kinds used by the operator visualizer.
 *
 * Mapping:
 *   { session }  -> user      (session-scope status events initiated around the operator)
 *   { run }      -> agent     (agent-level run status updates)
 *   { approval } -> tool_call or tool_result, depending on phase
 *   { artifact } -> tool_result (outputs produced by the agent)
 *   { runReconciledOnStartup } -> tool_result (daemon recovery diagnostic)
 *   { tokenUsageRecorded } -> tool_result (provider usage telemetry)
 *   { conflict } -> tool_result (scheduler warning surfaced to the operator)
 */
export function formatActivityLine(item: ActivityPageItem): ActivityLine {
  const key = cursorKey(item.cursor.sequence);
  const occurredAtMs = numberFromBig(item.occurredAtMs);
  const line = describeDaemonEvent(item.event);
  return {
    kind: line.kind,
    key,
    occurredAtMs,
    text: formatLineText(line.kind, line.text),
  };
}

export function formatAgentTurnLine(row: AgentTurnRow): ActivityLine {
  if (row.kind === "assistant") {
    return {
      kind: "agent",
      key: `assistant:${agentTurnLogicalKey(row)}`,
      occurredAtMs: numberFromBig(row.startedAtMs),
      text: formatLineText("agent", row.text.trim()),
    };
  }

  if (row.kind === "toolCall") {
    return {
      kind: "tool_result",
      key: `tool:${agentTurnLogicalKey(row)}`,
      occurredAtMs: numberFromBig(row.startedAtMs),
      text: formatLineText("tool_result", describeCommittedToolCall(row)),
    };
  }

  return {
    kind: "tool_result",
    key: `pending:${agentTurnLogicalKey(row)}`,
    occurredAtMs: numberFromBig(row.occurredAtMs),
    text: formatLineText("tool_result", `pending ${row.state}`),
  };
}

export function formatLiveAgentTurnLines(
  liveMessages: LiveAgentMessage[],
  liveToolCalls: LiveAgentToolCall[],
  committedRows: AgentTurnRow[],
): ActivityLine[] {
  const committedKeys = new Set(committedRows.map(agentTurnLogicalKey));
  const lines = [
    ...liveMessages
      .filter((message) => !committedKeys.has(assistantLogicalKey(message.runId, message.turnId)))
      .map((message) => ({
        kind: "agent" as const,
        key: `live-agent:${assistantLogicalKey(message.runId, message.turnId)}:${message.lastSequence.toString()}`,
        occurredAtMs: numberFromBig(message.startedAtMs),
        text: formatLineText("agent", describeLiveAssistantMessage(message)),
      })),
    ...liveToolCalls
      .filter(
        (toolCall) =>
          (toolCall.output.trim().length > 0 || toolCall.outcome !== null) &&
          !committedKeys.has(toolLogicalKey(toolCall.runId, toolCall.turnId, toolCall.itemId)),
      )
      .map((toolCall) => {
        const kind = toolCall.outcome === null ? ("tool_call" as const) : ("tool_result" as const);
        return {
          kind,
          key: `live-tool:${toolLogicalKey(toolCall.runId, toolCall.turnId, toolCall.itemId)}:${toolCall.lastSequence.toString()}`,
          occurredAtMs: numberFromBig(toolCall.startedAtMs),
          text: formatLineText(kind, describeLiveToolCall(toolCall)),
        };
      }),
  ];

  return sortActivityLinesAscending(lines);
}

function describeLiveAssistantMessage(message: LiveAgentMessage): string {
  const text = message.text.trim();
  if (text.length > 0) {
    return text;
  }
  return message.completed ? "..." : "thinking...";
}

interface DescribedEvent {
  kind: ActivityLineKind;
  text: string;
}

function describeDaemonEvent(event: ActivityPageItem["event"]): DescribedEvent {
  if ("session" in event) {
    return {
      kind: "user",
      text: `session ${event.session.status}`,
    };
  }
  if ("run" in event) {
    return {
      kind: "agent",
      text: `run ${event.run.status} ${truncate(event.run.detail, 140)}`.trim(),
    };
  }
  if ("approval" in event) {
    const approval = event.approval;
    if (approval.phase === "requested") {
      return {
        kind: "tool_call",
        text: `approval.${approval.request.scope}(run=${approval.request.runId})`,
      };
    }
    const outcome = approval.resolution.decision === "approved" ? "ok" : "err";
    return {
      kind: "tool_result",
      text: `${outcome} approval.${approval.resolution.decision} (run=${approval.resolution.runId})`,
    };
  }
  if ("artifact" in event) {
    return {
      kind: "tool_result",
      text: `ok artifact ${event.artifact.artifact.kind} ${event.artifact.artifact.storagePath}`,
    };
  }
  if ("contextReceipt" in event) {
    const { phase, receipt } = event.contextReceipt;
    return {
      kind: "tool_result",
      text: `ok receipt.${phase} ${receipt.kind} ${receipt.state}`,
    };
  }
  if ("agentStream" in event) {
    return describeAgentStreamLine(event.agentStream);
  }
  if ("runReconciledOnStartup" in event) {
    return {
      kind: "tool_result",
      text: `warn restart.reconciled (run=${event.runReconciledOnStartup.runId}, reason=${event.runReconciledOnStartup.reason})`,
    };
  }
  if ("tokenUsageRecorded" in event) {
    const usage = event.tokenUsageRecorded;
    return {
      kind: "tool_result",
      text: `ok tokens (run=${usage.runId}, prompt=${usage.promptTokens}, completion=${usage.completionTokens})`,
    };
  }
  if ("conflict" in event) {
    return {
      kind: "tool_result",
      text: `warn conflict.${event.conflict.warning.severity} (run=${event.conflict.run_id}, files=${event.conflict.warning.conflicts.length})`,
    };
  }
  if ("budget" in event) {
    const budgetEvent = event.budget.event;
    return {
      kind: "tool_result",
      text: `err budget.${budgetEvent.breach.metric} (run=${budgetEvent.runId}, scope=${budgetEvent.breach.scope})`,
    };
  }
  const exhaustive: never = event;
  throw new Error(`unhandled daemon activity event: ${JSON.stringify(exhaustive)}`);
}

/**
 * Maps a single `AgentStreamEvent` to a compact terminal line for the
 * phosphor-decay tail in AgentRunStream.
 *
 * Tool-call events correlate via {@link AgentStreamEvent.itemId} (not
 * timestamps). The mapping covers every current AgentStreamFrame variant:
 * assistantTurnStarted/MessageDelta/TurnCompleted,
 * toolCallStarted/Progressed/Completed, pendingStateChanged, tokenUsageUpdated.
 */
export function describeAgentStreamLine(event: AgentStreamEvent): DescribedEvent {
  const itemSuffix = agentStreamItemSuffix(event.itemId);
  const frame = event.frame;
  switch (frame.kind) {
    case "assistantTurnStarted":
      return { kind: "agent", text: "assistant turn started" };
    case "assistantMessageDelta": {
      const trimmed = frame.delta.trim();
      return {
        kind: "agent",
        text: trimmed.length > 0 ? truncate(trimmed, 160) : "(…)",
      };
    }
    case "assistantTurnCompleted":
      return { kind: "agent", text: "assistant turn completed" };
    case "toolCallStarted":
      return {
        kind: "tool_call",
        text: `start ${frame.toolName}${itemSuffix}`,
      };
    case "toolCallProgressed": {
      const trimmed = frame.delta.trim();
      const progress = trimmed.length > 0 ? ` ${truncate(trimmed, 120)}` : "";
      return { kind: "tool_call", text: `progress${itemSuffix}${progress}` };
    }
    case "toolCallCompleted":
      return {
        kind: "tool_result",
        text: `${describeToolCallOutcome(frame.outcome)}${itemSuffix}`,
      };
    case "pendingStateChanged":
      return {
        kind: "agent",
        text: `runtime ${describeRuntimeLanePendingState(frame.state)}`,
      };
    case "tokenUsageUpdated":
      return {
        kind: "agent",
        text: `tokens total=${frame.totalTokens ?? "unknown"} context=${frame.modelContextWindow ?? "unknown"}`,
      };
    default: {
      const exhaustive: never = frame;
      throw new Error(`unhandled AgentStreamFrame variant: ${JSON.stringify(exhaustive)}`);
    }
  }
}

function agentStreamItemSuffix(itemId: AgentStreamItemId | null | undefined): string {
  if (itemId === null || itemId === undefined) {
    return "";
  }
  return ` item=${itemId}`;
}

function describeToolCallOutcome(outcome: AgentToolCallOutcome): string {
  switch (outcome) {
    case "completed":
      return "ok";
    case "failed":
      return "err";
    case "cancelled":
      return "cancelled";
  }
}

function describeRuntimeLanePendingState(state: RuntimeLanePendingState): string {
  switch (state) {
    case "queued":
      return "queued";
    case "waitingForApproval":
      return "waiting for approval";
    case "waitingForInput":
      return "waiting for input";
  }
}

/** Re-exported variant union enumerated for exhaustive coverage in tests. */
export type AgentStreamFrameKind = AgentStreamFrame["kind"];

function formatLineText(kind: ActivityLineKind, text: string): string {
  const trimmed = text.trim();
  switch (kind) {
    case "user":
      return `> user:       ${trimmed}`;
    case "agent":
      return `> agent:      ${trimmed}`;
    case "tool_call":
      return `> tool_call:  ${trimmed}`;
    case "tool_result":
      return `> tool_result: ${trimmed}`;
  }
}

function truncate(value: string, max: number): string {
  if (value.length <= max) {
    return value;
  }
  return `${value.slice(0, Math.max(0, max - 1))}…`;
}

function numberFromBig(value: bigint | number): number {
  return typeof value === "bigint" ? Number(value) : value;
}

function cursorKey(sequence: bigint | number): string {
  return typeof sequence === "bigint" ? sequence.toString() : String(sequence);
}

export function sortActivityAscending(items: ActivityPageItem[]): ActivityPageItem[] {
  return [...items].sort((left, right) => {
    const leftSequence = left.cursor.sequence;
    const rightSequence = right.cursor.sequence;
    if (leftSequence === rightSequence) {
      return 0;
    }
    return rightSequence > leftSequence ? -1 : 1;
  });
}

export function sortActivityLinesAscending(lines: ActivityLine[]): ActivityLine[] {
  return [...lines].sort((left, right) => {
    if (left.occurredAtMs === right.occurredAtMs) {
      return left.key.localeCompare(right.key);
    }
    return left.occurredAtMs - right.occurredAtMs;
  });
}

export function describeRunStatus(status: RunStatus): string {
  switch (status) {
    case "queued":
      return "queued";
    case "running":
      return "running";
    case "waitingForApproval":
      return "waiting";
    case "completed":
      return "completed";
    case "failed":
      return "failed";
    case "budgetExceeded":
      return "budget exceeded";
    case "cancelled":
      return "cancelled";
  }
}

export function splitLatestAndOlderRuns(runs: RunSummary[]): {
  latest: RunSummary | null;
  older: RunSummary[];
} {
  if (runs.length === 0) {
    return { latest: null, older: [] };
  }
  const [latest, ...older] = runs;
  return { latest, older };
}

export function describeApprovalReason(approval: ApprovalRequest): string {
  return approval.reason.length === 0 ? "(no reason provided)" : approval.reason;
}

/**
 * Canonical wording for a missing-artifact outcome, shared across
 * ArtifactsSection and ArtifactViewer so the UX stays consistent whether
 * the missing state originates from the list-level save flow or the
 * per-row content viewer.
 */
export function describeArtifactMissingReason(reason: "artifactNotFound" | "fileNotFound"): string {
  switch (reason) {
    case "artifactNotFound":
      return "artifact no longer exists";
    case "fileNotFound":
      return "artifact file is missing on disk";
  }
}

function describeLiveToolCall(toolCall: LiveAgentToolCall): string {
  const toolName = toolCall.toolName ?? "tool";
  const progress = toolCall.output.trim();
  if (toolCall.outcome === null) {
    return progress.length > 0 ? `${toolName} ${progress}` : toolName;
  }

  const prefix =
    toolCall.outcome === "completed" ? "ok" : toolCall.outcome === "failed" ? "err" : "cancelled";
  return progress.length > 0 ? `${prefix} ${toolName} ${progress}` : `${prefix} ${toolName}`;
}

function describeCommittedToolCall(row: Extract<AgentTurnRow, { kind: "toolCall" }>): string {
  const progress = row.output.trim();
  const prefix =
    row.outcome === "completed" ? "ok" : row.outcome === "failed" ? "err" : "cancelled";
  return progress.length > 0
    ? `${prefix} ${row.toolName} ${progress}`
    : `${prefix} ${row.toolName}`;
}

function agentTurnLogicalKey(row: AgentTurnRow): string {
  if (row.kind === "assistant") {
    return assistantLogicalKey(row.runId, row.turnId ?? null);
  }
  if (row.kind === "toolCall") {
    return toolLogicalKey(row.runId, row.turnId ?? null, row.itemId ?? null);
  }
  return `pending:${row.runId}:${row.turnId ?? "__turn__"}:${row.state}`;
}
