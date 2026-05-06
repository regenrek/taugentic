import { describe, expect, it } from "vite-plus/test";

import {
  createInitialSessionAgentStreamState,
  reduceAgentStreamMessage,
} from "../../packages/renderer/src/features/agent-stream/index.js";

describe("agent stream state", () => {
  it("appends assistant deltas into a single live message per turn", () => {
    const initial = createInitialSessionAgentStreamState("session-1");

    const afterFirst = reduceAgentStreamMessage(initial, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 2n,
      occurredAtMs: 20n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: null,
          fragmentSequence: 1n,
          frame: {
            kind: "assistantMessageDelta",
            delta: "hello ",
          },
        },
      },
    }).state;

    const afterSecond = reduceAgentStreamMessage(afterFirst, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 3n,
      occurredAtMs: 30n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: null,
          fragmentSequence: 2n,
          frame: {
            kind: "assistantMessageDelta",
            delta: "world",
          },
        },
      },
    }).state;

    expect(afterSecond.liveMessages).toEqual([
      {
        completed: false,
        firstSequence: 2n,
        lastSequence: 3n,
        occurredAtMs: 30n,
        runId: "run-1",
        startedAtMs: 20n,
        text: "hello world",
        turnId: "turn-1",
      },
    ]);
  });

  it("keeps tool progress and marks completion as a durable refresh boundary", () => {
    const initial = createInitialSessionAgentStreamState("session-1");

    const afterStart = reduceAgentStreamMessage(initial, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 4n,
      occurredAtMs: 40n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: "item-1",
          fragmentSequence: 1n,
          frame: {
            kind: "toolCallStarted",
            toolName: "shell",
            input: '{"cmd":"echo hi"}',
          },
        },
      },
    }).state;

    const afterProgress = reduceAgentStreamMessage(afterStart, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 5n,
      occurredAtMs: 50n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: "item-1",
          fragmentSequence: 2n,
          frame: {
            kind: "toolCallProgressed",
            delta: "echo hi",
          },
        },
      },
    }).state;

    const completed = reduceAgentStreamMessage(afterProgress, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 6n,
      occurredAtMs: 60n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: "item-1",
          fragmentSequence: 3n,
          frame: {
            kind: "toolCallCompleted",
            outcome: "completed",
          },
        },
      },
    });

    expect(completed.needsCommittedRefresh).toBe(true);
    expect(completed.state.liveToolCalls).toEqual([
      {
        firstSequence: 4n,
        itemId: "item-1",
        lastSequence: 6n,
        occurredAtMs: 60n,
        outcome: "completed",
        output: "echo hi",
        runId: "run-1",
        startedAtMs: 40n,
        toolName: "shell",
        turnId: "turn-1",
      },
    ]);
  });

  it("upserts a completed assistant placeholder when no live delta arrived first", () => {
    const initial = createInitialSessionAgentStreamState("session-1");

    const completed = reduceAgentStreamMessage(initial, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 7n,
      occurredAtMs: 70n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: null,
          fragmentSequence: 1n,
          frame: {
            kind: "assistantTurnCompleted",
          },
        },
      },
    });

    expect(completed.needsCommittedRefresh).toBe(true);
    expect(completed.state.liveMessages).toEqual([
      {
        completed: true,
        firstSequence: 7n,
        lastSequence: 7n,
        occurredAtMs: 70n,
        runId: "run-1",
        startedAtMs: 70n,
        text: "",
        turnId: "turn-1",
      },
    ]);
  });

  it("upserts a completed tool placeholder when no live progress arrived first", () => {
    const initial = createInitialSessionAgentStreamState("session-1");

    const completed = reduceAgentStreamMessage(initial, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 8n,
      occurredAtMs: 80n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: "item-1",
          fragmentSequence: 1n,
          frame: {
            kind: "toolCallCompleted",
            outcome: "completed",
          },
        },
      },
    });

    expect(completed.needsCommittedRefresh).toBe(true);
    expect(completed.state.liveToolCalls).toEqual([
      {
        firstSequence: 8n,
        itemId: "item-1",
        lastSequence: 8n,
        occurredAtMs: 80n,
        outcome: "completed",
        output: "",
        runId: "run-1",
        startedAtMs: 80n,
        toolName: null,
        turnId: "turn-1",
      },
    ]);
  });

  it("clears live buffers immediately on historyGap", () => {
    const initial = createInitialSessionAgentStreamState("session-1");
    const withDelta = reduceAgentStreamMessage(initial, {
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 2n,
      occurredAtMs: 20n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: null,
          fragmentSequence: 1n,
          frame: {
            kind: "assistantMessageDelta",
            delta: "partial",
          },
        },
      },
    }).state;

    const afterGap = reduceAgentStreamMessage(withDelta, {
      stream: "agentStream",
      status: "historyGap",
    });

    expect(afterGap.hasHistoryGap).toBe(true);
    expect(afterGap.needsCommittedRefresh).toBe(true);
    expect(afterGap.state.liveMessages).toEqual([]);
    expect(afterGap.state.liveToolCalls).toEqual([]);
  });
});
