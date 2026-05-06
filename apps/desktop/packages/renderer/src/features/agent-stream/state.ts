import type {
  AgentTurnRow,
  AgentTurnsPageResult,
  AgentStreamMessage,
  AgentToolCallOutcome,
  SessionId,
} from "@taugentic/desktop-shared";

export interface LiveAgentMessage {
  completed: boolean;
  firstSequence: bigint;
  lastSequence: bigint;
  occurredAtMs: bigint;
  runId: string;
  startedAtMs: bigint;
  text: string;
  turnId: string | null;
}

export interface LiveAgentToolCall {
  firstSequence: bigint;
  itemId: string | null;
  lastSequence: bigint;
  occurredAtMs: bigint;
  outcome: AgentToolCallOutcome | null;
  output: string;
  runId: string;
  startedAtMs: bigint;
  toolName: string | null;
  turnId: string | null;
}

export interface SessionAgentStreamState {
  committedRows: AgentTurnRow[];
  errorMessage: string | null;
  hasHydratedCommitted: boolean;
  liveMessages: LiveAgentMessage[];
  liveToolCalls: LiveAgentToolCall[];
  sessionId: SessionId;
  streamStatus:
    | "connecting"
    | "ready"
    | "recoveringFromGap"
    | "rehydratingCommitted"
    | "reopeningLiveStream"
    | "error";
}

export interface ReducedAgentStreamMessage {
  hasHistoryGap: boolean;
  needsCommittedRefresh: boolean;
  state: SessionAgentStreamState;
}

export function createInitialSessionAgentStreamState(
  sessionId: SessionId,
): SessionAgentStreamState {
  return {
    committedRows: [],
    errorMessage: null,
    hasHydratedCommitted: false,
    liveMessages: [],
    liveToolCalls: [],
    sessionId,
    streamStatus: "connecting",
  };
}

export function clearLiveOverlay(state: SessionAgentStreamState): SessionAgentStreamState {
  return {
    ...state,
    liveMessages: [],
    liveToolCalls: [],
  };
}

export function hydrateCommittedAgentTurns(
  state: SessionAgentStreamState,
  snapshot: AgentTurnsPageResult,
): SessionAgentStreamState {
  const committedRows = snapshot.items ?? [];
  const committedAssistantKeys = new Set<string>();
  const committedToolKeys = new Set<string>();

  for (const row of committedRows) {
    if (row.kind === "assistant") {
      committedAssistantKeys.add(assistantLogicalKey(row.runId, row.turnId ?? null));
      continue;
    }
    if (row.kind === "toolCall") {
      committedToolKeys.add(toolLogicalKey(row.runId, row.turnId ?? null, row.itemId ?? null));
    }
  }

  return {
    ...state,
    committedRows,
    hasHydratedCommitted: true,
    liveMessages: state.liveMessages.filter(
      (message) => !committedAssistantKeys.has(assistantLogicalKey(message.runId, message.turnId)),
    ),
    liveToolCalls: state.liveToolCalls.filter(
      (toolCall) =>
        !committedToolKeys.has(toolLogicalKey(toolCall.runId, toolCall.turnId, toolCall.itemId)),
    ),
  };
}

export function reduceAgentStreamMessage(
  state: SessionAgentStreamState,
  message: AgentStreamMessage,
): ReducedAgentStreamMessage {
  if ("status" in message) {
    switch (message.status) {
      case "ready":
        return {
          hasHistoryGap: false,
          needsCommittedRefresh: false,
          state: {
            ...state,
            errorMessage: null,
            streamStatus: "ready",
          },
        };
      case "historyGap":
        return {
          hasHistoryGap: true,
          needsCommittedRefresh: true,
          state: {
            ...clearLiveOverlay(state),
            errorMessage: null,
            streamStatus: "recoveringFromGap",
          },
        };
      case "terminalError":
        return {
          hasHistoryGap: false,
          needsCommittedRefresh: false,
          state: {
            ...state,
            errorMessage: `agent stream entered a terminal error state for ${state.sessionId}`,
            streamStatus: "error",
          },
        };
    }
  }

  if (!("agentStream" in message.event)) {
    return {
      hasHistoryGap: false,
      needsCommittedRefresh: false,
      state,
    };
  }

  const agentStream = message.event.agentStream;
  const frame = agentStream.frame;
  let nextState: SessionAgentStreamState = {
    ...state,
    errorMessage: null,
    streamStatus: "ready",
  };
  let needsCommittedRefresh = false;

  switch (frame.kind) {
    case "assistantTurnStarted":
      nextState = {
        ...nextState,
        liveMessages: upsertLiveMessage(nextState.liveMessages, {
          completed: false,
          firstSequence: message.sequence,
          lastSequence: message.sequence,
          occurredAtMs: message.occurredAtMs,
          runId: agentStream.runId,
          startedAtMs: message.occurredAtMs,
          text: "",
          turnId: agentStream.turnId ?? null,
        }),
      };
      break;
    case "assistantMessageDelta":
      if (frame.delta.length === 0) {
        return {
          hasHistoryGap: false,
          needsCommittedRefresh: false,
          state: nextState,
        };
      }
      nextState = {
        ...nextState,
        liveMessages: upsertLiveMessage(nextState.liveMessages, {
          completed: false,
          firstSequence: message.sequence,
          lastSequence: message.sequence,
          occurredAtMs: message.occurredAtMs,
          runId: agentStream.runId,
          startedAtMs: message.occurredAtMs,
          text: frame.delta,
          turnId: agentStream.turnId ?? null,
        }),
      };
      break;
    case "assistantTurnCompleted":
      nextState = {
        ...nextState,
        liveMessages: markLiveMessageCompleted(
          nextState.liveMessages,
          agentStream.runId,
          agentStream.turnId ?? null,
          message.sequence,
          message.occurredAtMs,
        ),
      };
      needsCommittedRefresh = true;
      break;
    case "toolCallStarted":
      nextState = {
        ...nextState,
        liveToolCalls: upsertLiveToolCall(nextState.liveToolCalls, {
          firstSequence: message.sequence,
          itemId: agentStream.itemId ?? null,
          lastSequence: message.sequence,
          occurredAtMs: message.occurredAtMs,
          outcome: null,
          output: "",
          runId: agentStream.runId,
          startedAtMs: message.occurredAtMs,
          toolName: frame.toolName,
          turnId: agentStream.turnId ?? null,
        }),
      };
      break;
    case "toolCallProgressed":
      if (frame.delta.length === 0) {
        return {
          hasHistoryGap: false,
          needsCommittedRefresh: false,
          state: nextState,
        };
      }
      nextState = {
        ...nextState,
        liveToolCalls: upsertLiveToolCall(nextState.liveToolCalls, {
          firstSequence: message.sequence,
          itemId: agentStream.itemId ?? null,
          lastSequence: message.sequence,
          occurredAtMs: message.occurredAtMs,
          outcome: null,
          output: frame.delta,
          runId: agentStream.runId,
          startedAtMs: message.occurredAtMs,
          toolName: null,
          turnId: agentStream.turnId ?? null,
        }),
      };
      break;
    case "toolCallCompleted":
      nextState = {
        ...nextState,
        liveToolCalls: markLiveToolCallCompleted(
          nextState.liveToolCalls,
          agentStream.runId,
          agentStream.turnId ?? null,
          agentStream.itemId ?? null,
          frame.outcome,
          message.sequence,
          message.occurredAtMs,
        ),
      };
      needsCommittedRefresh = true;
      break;
    case "pendingStateChanged":
      needsCommittedRefresh = true;
      break;
    case "tokenUsageUpdated":
      needsCommittedRefresh = false;
      break;
  }

  return {
    hasHistoryGap: false,
    needsCommittedRefresh,
    state: nextState,
  };
}

export function toAgentStreamErrorMessage(sessionId: SessionId, error: unknown): string {
  if (error instanceof Error) {
    return `agent stream failed for ${sessionId}: ${error.message}`;
  }
  return `agent stream failed for ${sessionId}: ${String(error)}`;
}

export function assistantLogicalKey(runId: string, turnId: string | null): string {
  return `${runId}:${turnId ?? "__turn__"}`;
}

export function toolLogicalKey(
  runId: string,
  turnId: string | null,
  itemId: string | null,
): string {
  return `${runId}:${turnId ?? "__turn__"}:${itemId ?? "__item__"}`;
}

function upsertLiveMessage(
  messages: LiveAgentMessage[],
  incoming: LiveAgentMessage,
): LiveAgentMessage[] {
  const key = assistantLogicalKey(incoming.runId, incoming.turnId);
  const index = messages.findIndex(
    (message) => assistantLogicalKey(message.runId, message.turnId) === key,
  );
  if (index < 0) {
    return [...messages, incoming];
  }

  const current = messages[index]!;
  if (incoming.lastSequence <= current.lastSequence) {
    return messages;
  }

  const next = [...messages];
  next[index] = {
    ...current,
    completed: current.completed || incoming.completed,
    lastSequence: incoming.lastSequence,
    occurredAtMs: incoming.occurredAtMs,
    text: `${current.text}${incoming.text}`,
  };
  return next;
}

function markLiveMessageCompleted(
  messages: LiveAgentMessage[],
  runId: string,
  turnId: string | null,
  sequence: bigint,
  occurredAtMs: bigint,
): LiveAgentMessage[] {
  const key = assistantLogicalKey(runId, turnId);
  const index = messages.findIndex(
    (message) => assistantLogicalKey(message.runId, message.turnId) === key,
  );
  if (index < 0) {
    return [
      ...messages,
      {
        completed: true,
        firstSequence: sequence,
        lastSequence: sequence,
        occurredAtMs,
        runId,
        startedAtMs: occurredAtMs,
        text: "",
        turnId,
      },
    ];
  }

  const current = messages[index]!;
  if (sequence <= current.lastSequence && current.completed) {
    return messages;
  }

  const next = [...messages];
  next[index] = {
    ...current,
    completed: true,
    lastSequence: current.lastSequence > sequence ? current.lastSequence : sequence,
    occurredAtMs,
  };
  return next;
}

function upsertLiveToolCall(
  toolCalls: LiveAgentToolCall[],
  incoming: LiveAgentToolCall,
): LiveAgentToolCall[] {
  const key = toolLogicalKey(incoming.runId, incoming.turnId, incoming.itemId);
  const index = toolCalls.findIndex(
    (toolCall) => toolLogicalKey(toolCall.runId, toolCall.turnId, toolCall.itemId) === key,
  );
  if (index < 0) {
    return [...toolCalls, incoming];
  }

  const current = toolCalls[index]!;
  if (incoming.lastSequence <= current.lastSequence) {
    return toolCalls;
  }

  const next = [...toolCalls];
  next[index] = {
    ...current,
    lastSequence: incoming.lastSequence,
    occurredAtMs: incoming.occurredAtMs,
    outcome: incoming.outcome ?? current.outcome,
    output: `${current.output}${incoming.output}`,
    toolName: incoming.toolName ?? current.toolName,
  };
  return next;
}

function markLiveToolCallCompleted(
  toolCalls: LiveAgentToolCall[],
  runId: string,
  turnId: string | null,
  itemId: string | null,
  outcome: AgentToolCallOutcome,
  sequence: bigint,
  occurredAtMs: bigint,
): LiveAgentToolCall[] {
  const key = toolLogicalKey(runId, turnId, itemId);
  const index = toolCalls.findIndex(
    (toolCall) => toolLogicalKey(toolCall.runId, toolCall.turnId, toolCall.itemId) === key,
  );
  if (index < 0) {
    return [
      ...toolCalls,
      {
        firstSequence: sequence,
        itemId,
        lastSequence: sequence,
        occurredAtMs,
        outcome,
        output: "",
        runId,
        startedAtMs: occurredAtMs,
        toolName: null,
        turnId,
      },
    ];
  }

  const current = toolCalls[index]!;
  if (sequence <= current.lastSequence && current.outcome === outcome) {
    return toolCalls;
  }

  const next = [...toolCalls];
  next[index] = {
    ...current,
    lastSequence: current.lastSequence > sequence ? current.lastSequence : sequence,
    occurredAtMs,
    outcome,
  };
  return next;
}
