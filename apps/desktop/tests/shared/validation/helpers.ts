export function daemonCursor(
  sequence: string | bigint,
  sessionId = "session-7",
  daemonInstanceId = "daemon-1",
) {
  return {
    daemonInstanceId,
    sessionId,
    sequence,
  };
}

export function agentStreamEnvelope(frame: Record<string, unknown>) {
  return {
    daemonInstanceId: "daemon-1",
    sessionId: "session-1",
    sequence: "44",
    occurredAtMs: "101",
    event: {
      agentStream: {
        runId: "run-1",
        turnId: "turn-1",
        itemId: "item-1",
        fragmentSequence: 3,
        frame,
      },
    },
  };
}

export function agentStreamActivityItem(frame: Record<string, unknown>) {
  return {
    cursor: {
      sequence: "44",
    },
    occurredAtMs: "101",
    event: agentStreamEnvelope(frame).event,
  };
}

export function agentTurnAssistantItem() {
  return {
    kind: "assistant",
    cursor: {
      sequence: "45",
    },
    sessionId: "session-1",
    runId: "run-1",
    turnId: "turn-1",
    startedAtMs: "100",
    completedAtMs: "120",
    text: "hello world",
  };
}

export function agentTurnToolCallItem() {
  return {
    kind: "toolCall",
    cursor: {
      sequence: "46",
    },
    sessionId: "session-1",
    runId: "run-1",
    turnId: "turn-1",
    itemId: "item-1",
    toolName: "shell",
    input: '{"cmd":"echo hi"}',
    output: "echo hi",
    outcome: "completed",
    startedAtMs: "110",
    completedAtMs: "120",
  };
}
