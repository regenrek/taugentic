import type {
  AgentAssistantRow,
  AgentPendingStateRow,
  AgentToolCallRow,
  AgentTurnRow,
  AgentTurnsPageQuery,
  AgentTurnsPageResult,
  SessionId,
} from "../contracts.js";
import { PROTOCOL_JSON_SCHEMAS } from "../../generated/runtime.js";
import {
  ajv,
  formatProtocolValidationErrors,
  parseNonEmptyProtocolString,
  parseNullableBoundaryValue,
  parseProtocolBigInt,
  parseSchema,
  parseStringField,
  ProtocolValidationError,
} from "./core.js";
import { parseActivityCursor, parseDaemonEventCursor } from "./cursors.js";
import { parseSessionId } from "./identity.js";

const validateAgentTurnsPageQuery = ajv.compile(PROTOCOL_JSON_SCHEMAS.AgentTurnsPageQuery);
const validateAgentTurnsPageResult = ajv.compile(PROTOCOL_JSON_SCHEMAS.AgentTurnsPageResult);

export function parseAgentTurnsPageQuery(value: unknown): AgentTurnsPageQuery {
  const record = parseSchema<{
    limit: number;
    before?: unknown;
  }>("AgentTurnsPageQuery", validateAgentTurnsPageQuery, value);
  return {
    limit: record.limit,
    before: parseNullableBoundaryValue(record.before, (before) =>
      parseActivityCursor(before, "AgentTurnsPageQuery.before"),
    ),
  };
}

export function parseAgentTurnsPageResult(value: unknown): AgentTurnsPageResult {
  if (validateAgentTurnsPageResult(value)) {
    const record = value as {
      items?: unknown[];
      nextBefore?: unknown;
      latestCursor?: unknown;
    };
    return {
      items: (record.items ?? []).map((item) => parseAgentTurnRow(item)),
      nextBefore: parseNullableBoundaryValue(record.nextBefore, (nextBefore) =>
        parseActivityCursor(nextBefore, "AgentTurnsPageResult.nextBefore"),
      ),
      latestCursor: parseNullableBoundaryValue(record.latestCursor, (latestCursor) =>
        parseDaemonEventCursor(latestCursor, "AgentTurnsPageResult.latestCursor"),
      ),
    };
  }

  throw new ProtocolValidationError(
    formatProtocolValidationErrors("AgentTurnsPageResult", validateAgentTurnsPageResult.errors),
  );
}

function parseAgentTurnRow(value: unknown): AgentTurnRow {
  if (typeof value !== "object" || value === null || !("kind" in value)) {
    throw new ProtocolValidationError("AgentTurnRow failed protocol validation: missing kind");
  }

  const kind = (value as { kind?: unknown }).kind;
  if (kind === "assistant") {
    return {
      kind,
      ...parseAgentAssistantRow(value),
    };
  }
  if (kind === "toolCall") {
    return {
      kind,
      ...parseAgentToolCallRow(value),
    };
  }
  if (kind === "pendingState") {
    return {
      kind,
      ...parseAgentPendingStateRow(value),
    };
  }

  throw new ProtocolValidationError(
    "AgentTurnRow.kind must be assistant, toolCall, or pendingState",
  );
}

function parseAgentAssistantRow(value: unknown): Omit<AgentAssistantRow, "kind"> {
  if (
    typeof value !== "object" ||
    value === null ||
    !("cursor" in value) ||
    !("sessionId" in value) ||
    !("runId" in value) ||
    !("startedAtMs" in value) ||
    !("completedAtMs" in value) ||
    !("text" in value)
  ) {
    throw new ProtocolValidationError(
      "AgentAssistantRow failed protocol validation: missing required fields",
    );
  }

  const record = value as {
    cursor: unknown;
    sessionId: SessionId;
    runId: string;
    turnId?: string | null;
    startedAtMs: string;
    completedAtMs: string;
    text: string;
  };
  return {
    cursor: parseActivityCursor(record.cursor, "AgentAssistantRow.cursor"),
    sessionId: parseSessionId(record.sessionId),
    runId: parseNonEmptyProtocolString(record.runId, "AgentAssistantRow.runId"),
    turnId: parseNullableBoundaryValue(record.turnId, (turnId) =>
      parseNonEmptyProtocolString(turnId, "AgentAssistantRow.turnId"),
    ),
    startedAtMs: parseProtocolBigInt(record.startedAtMs, "AgentAssistantRow.startedAtMs"),
    completedAtMs: parseProtocolBigInt(record.completedAtMs, "AgentAssistantRow.completedAtMs"),
    text: parseStringField(record.text, "AgentAssistantRow.text"),
  };
}

function parseAgentToolCallRow(value: unknown): Omit<AgentToolCallRow, "kind"> {
  if (
    typeof value !== "object" ||
    value === null ||
    !("cursor" in value) ||
    !("sessionId" in value) ||
    !("runId" in value) ||
    !("toolName" in value) ||
    !("input" in value) ||
    !("output" in value) ||
    !("outcome" in value) ||
    !("startedAtMs" in value) ||
    !("completedAtMs" in value)
  ) {
    throw new ProtocolValidationError(
      "AgentToolCallRow failed protocol validation: missing required fields",
    );
  }

  const record = value as {
    cursor: unknown;
    sessionId: SessionId;
    runId: string;
    turnId?: string | null;
    itemId?: string | null;
    toolName: string;
    input: string;
    output: string;
    outcome: AgentToolCallRow["outcome"];
    startedAtMs: string;
    completedAtMs: string;
  };
  return {
    cursor: parseActivityCursor(record.cursor, "AgentToolCallRow.cursor"),
    sessionId: parseSessionId(record.sessionId),
    runId: parseNonEmptyProtocolString(record.runId, "AgentToolCallRow.runId"),
    turnId: parseNullableBoundaryValue(record.turnId, (turnId) =>
      parseNonEmptyProtocolString(turnId, "AgentToolCallRow.turnId"),
    ),
    itemId: parseNullableBoundaryValue(record.itemId, (itemId) =>
      parseNonEmptyProtocolString(itemId, "AgentToolCallRow.itemId"),
    ),
    toolName: parseStringField(record.toolName, "AgentToolCallRow.toolName"),
    input: parseStringField(record.input, "AgentToolCallRow.input"),
    output: parseStringField(record.output, "AgentToolCallRow.output"),
    outcome: record.outcome,
    startedAtMs: parseProtocolBigInt(record.startedAtMs, "AgentToolCallRow.startedAtMs"),
    completedAtMs: parseProtocolBigInt(record.completedAtMs, "AgentToolCallRow.completedAtMs"),
  };
}

function parseAgentPendingStateRow(value: unknown): Omit<AgentPendingStateRow, "kind"> {
  if (
    typeof value !== "object" ||
    value === null ||
    !("cursor" in value) ||
    !("sessionId" in value) ||
    !("runId" in value) ||
    !("occurredAtMs" in value) ||
    !("state" in value)
  ) {
    throw new ProtocolValidationError(
      "AgentPendingStateRow failed protocol validation: missing required fields",
    );
  }

  const record = value as {
    cursor: unknown;
    sessionId: SessionId;
    runId: string;
    turnId?: string | null;
    occurredAtMs: string;
    state: AgentPendingStateRow["state"];
  };
  return {
    cursor: parseActivityCursor(record.cursor, "AgentPendingStateRow.cursor"),
    sessionId: parseSessionId(record.sessionId),
    runId: parseNonEmptyProtocolString(record.runId, "AgentPendingStateRow.runId"),
    turnId: parseNullableBoundaryValue(record.turnId, (turnId) =>
      parseNonEmptyProtocolString(turnId, "AgentPendingStateRow.turnId"),
    ),
    occurredAtMs: parseProtocolBigInt(record.occurredAtMs, "AgentPendingStateRow.occurredAtMs"),
    state: record.state,
  };
}
