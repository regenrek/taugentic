import { describe, expect, it } from "vite-plus/test";

import type {
  AgentStreamEvent,
  AgentStreamFrame,
  PublicActivityPageItem,
} from "../../packages/shared/generated/index.js";
import {
  describeArtifactMissingReason,
  describeAgentStreamLine,
  formatActivityLine,
  type AgentStreamFrameKind,
} from "../../packages/renderer/src/features/session-detail/formatters.js";

function agentStreamEvent(
  frame: AgentStreamFrame,
  overrides: Partial<AgentStreamEvent> = {},
): AgentStreamEvent {
  return {
    runId: overrides.runId ?? "run-1",
    turnId: overrides.turnId ?? "turn-1",
    itemId: overrides.itemId ?? null,
    fragmentSequence: overrides.fragmentSequence ?? null,
    frame,
  };
}

function activityItem(event: AgentStreamEvent, sequence: bigint = 5n): PublicActivityPageItem {
  return {
    cursor: { sequence },
    occurredAtMs: 100n,
    event: { agentStream: event },
  };
}

describe("describeAgentStreamLine", () => {
  it("renders assistantTurnStarted as an agent-kind line", () => {
    const line = describeAgentStreamLine(agentStreamEvent({ kind: "assistantTurnStarted" }));
    expect(line.kind).toBe("agent");
    expect(line.text).toBe("assistant turn started");
  });

  it("renders assistantMessageDelta text (truncated if long)", () => {
    const line = describeAgentStreamLine(
      agentStreamEvent({ kind: "assistantMessageDelta", delta: "hello world" }),
    );
    expect(line.kind).toBe("agent");
    expect(line.text).toBe("hello world");
    const long = describeAgentStreamLine(
      agentStreamEvent({ kind: "assistantMessageDelta", delta: "x".repeat(400) }),
    );
    expect(long.text.length).toBeLessThanOrEqual(160);
  });

  it("falls back to (…) for an empty message delta", () => {
    const line = describeAgentStreamLine(
      agentStreamEvent({ kind: "assistantMessageDelta", delta: "   " }),
    );
    expect(line.text).toBe("(…)");
  });

  it("renders assistantTurnCompleted as an agent-kind line", () => {
    const line = describeAgentStreamLine(agentStreamEvent({ kind: "assistantTurnCompleted" }));
    expect(line.kind).toBe("agent");
    expect(line.text).toBe("assistant turn completed");
  });

  it("correlates toolCallStarted with itemId", () => {
    const line = describeAgentStreamLine(
      agentStreamEvent(
        { kind: "toolCallStarted", toolName: "shell", input: '{"cmd":"echo hi"}' },
        { itemId: "item-abc" },
      ),
    );
    expect(line.kind).toBe("tool_call");
    expect(line.text).toContain("start shell");
    expect(line.text).toContain("item=item-abc");
  });

  it("correlates toolCallProgressed with itemId and truncates long deltas", () => {
    const line = describeAgentStreamLine(
      agentStreamEvent(
        { kind: "toolCallProgressed", delta: "y".repeat(400) },
        { itemId: "item-123" },
      ),
    );
    expect(line.kind).toBe("tool_call");
    expect(line.text).toContain("progress");
    expect(line.text).toContain("item=item-123");
    expect(line.text.length).toBeLessThanOrEqual(150);
  });

  it("correlates toolCallCompleted outcome + itemId", () => {
    const completed = describeAgentStreamLine(
      agentStreamEvent({ kind: "toolCallCompleted", outcome: "completed" }, { itemId: "item-1" }),
    );
    expect(completed.kind).toBe("tool_result");
    expect(completed.text).toContain("ok");
    expect(completed.text).toContain("item=item-1");

    const failed = describeAgentStreamLine(
      agentStreamEvent({ kind: "toolCallCompleted", outcome: "failed" }),
    );
    expect(failed.text.startsWith("err")).toBe(true);

    const cancelled = describeAgentStreamLine(
      agentStreamEvent({ kind: "toolCallCompleted", outcome: "cancelled" }),
    );
    expect(cancelled.text.startsWith("cancelled")).toBe(true);
  });

  it("renders pendingStateChanged variants with readable labels", () => {
    expect(
      describeAgentStreamLine(agentStreamEvent({ kind: "pendingStateChanged", state: "queued" }))
        .text,
    ).toBe("runtime queued");
    expect(
      describeAgentStreamLine(
        agentStreamEvent({ kind: "pendingStateChanged", state: "waitingForApproval" }),
      ).text,
    ).toBe("runtime waiting for approval");
    expect(
      describeAgentStreamLine(
        agentStreamEvent({ kind: "pendingStateChanged", state: "waitingForInput" }),
      ).text,
    ).toBe("runtime waiting for input");
  });

  it("keeps every current AgentStreamFrame kind explicit", () => {
    const allKinds: readonly AgentStreamFrameKind[] = [
      "assistantTurnStarted",
      "assistantMessageDelta",
      "assistantTurnCompleted",
      "toolCallStarted",
      "toolCallProgressed",
      "toolCallCompleted",
      "pendingStateChanged",
    ];
    expect(allKinds.length).toBe(7);
  });
});

describe("formatActivityLine with agentStream items", () => {
  it("renders agentStream items through describeAgentStreamLine", () => {
    const line = formatActivityLine(
      activityItem(agentStreamEvent({ kind: "assistantTurnStarted" }), 12n),
    );
    expect(line.kind).toBe("agent");
    expect(line.key).toBe("12");
    expect(line.text).toContain("assistant turn started");
  });

  it("preserves lane-agnostic behavior across all frame variants", () => {
    const variants: AgentStreamFrame[] = [
      { kind: "assistantTurnStarted" },
      { kind: "assistantMessageDelta", delta: "hi" },
      { kind: "assistantTurnCompleted" },
      { kind: "toolCallStarted", toolName: "shell", input: '{"cmd":"echo hi"}' },
      { kind: "toolCallProgressed", delta: "out" },
      { kind: "toolCallCompleted", outcome: "completed" },
      { kind: "pendingStateChanged", state: "queued" },
    ];
    for (const frame of variants) {
      const line = formatActivityLine(activityItem(agentStreamEvent(frame)));
      expect(line.text.length).toBeGreaterThan(0);
      expect(["user", "agent", "tool_call", "tool_result"]).toContain(line.kind);
    }
  });
});

describe("describeArtifactMissingReason", () => {
  it("produces distinct lane-agnostic copy for each reason", () => {
    expect(describeArtifactMissingReason("artifactNotFound")).toBe("artifact no longer exists");
    expect(describeArtifactMissingReason("fileNotFound")).toBe("artifact file is missing on disk");
  });
});
