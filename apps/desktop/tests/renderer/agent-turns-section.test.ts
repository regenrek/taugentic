import { describe, expect, it, vi } from "vite-plus/test";

import type { AgentTurnRow } from "../../packages/shared/generated/index.js";
import type {
  LiveAgentMessage,
  LiveAgentToolCall,
} from "../../packages/renderer/src/features/agent-stream/index.js";

type CreateElementFn = (
  component: unknown,
  props?: Record<string, unknown> | null,
  ...children: unknown[]
) => unknown;
type RenderToStaticMarkupFn = (element: unknown) => string;

const reactModulePath = "../../packages/renderer/node_modules/react/index.js";
const reactServerModulePath = "../../packages/renderer/node_modules/react-dom/server.node.js";

const { createElement } = (await import(reactModulePath)) as {
  createElement: CreateElementFn;
};
const { renderToStaticMarkup } = (await import(reactServerModulePath)) as {
  renderToStaticMarkup: RenderToStaticMarkupFn;
};

const hooks = vi.hoisted(() => ({
  committedRows: [] as AgentTurnRow[],
  hasHydratedCommitted: true,
  liveMessages: [] as LiveAgentMessage[],
  liveToolCalls: [] as LiveAgentToolCall[],
  streamStatus: "ready" as const,
}));

vi.mock("../../packages/renderer/src/features/agent-stream/index.js", () => ({
  assistantLogicalKey: (runId: string, turnId: string | null) => `${runId}:${turnId ?? "__turn__"}`,
  toolLogicalKey: (runId: string, turnId: string | null, itemId: string | null) =>
    `${runId}:${turnId ?? "__turn__"}:${itemId ?? "__item__"}`,
  useAgentStream: () => ({
    committedRows: hooks.committedRows,
    errorMessage: null,
    hasHydratedCommitted: hooks.hasHydratedCommitted,
    liveMessages: hooks.liveMessages,
    liveToolCalls: hooks.liveToolCalls,
    streamStatus: hooks.streamStatus,
  }),
}));

const { AgentTurnsSection } =
  await import("../../packages/renderer/src/features/session-detail/AgentTurnsSection.js");

describe("AgentTurnsSection", () => {
  it("renders committed assistant, tool, and pending rows in chronological order", () => {
    hooks.committedRows = [
      {
        kind: "pendingState",
        cursor: { sequence: 1n },
        sessionId: "session-1",
        runId: "run-1",
        turnId: null,
        occurredAtMs: 1_000n,
        state: "waitingForApproval",
      },
      {
        kind: "assistant",
        cursor: { sequence: 2n },
        sessionId: "session-1",
        runId: "run-1",
        turnId: "turn-1",
        startedAtMs: 2_000n,
        completedAtMs: 2_400n,
        text: "objective started",
      },
      {
        kind: "toolCall",
        cursor: { sequence: 3n },
        sessionId: "session-1",
        runId: "run-1",
        turnId: "turn-1",
        itemId: "item-1",
        toolName: "shell",
        input: "echo hi",
        output: "echo hi",
        outcome: "completed",
        startedAtMs: 3_000n,
        completedAtMs: 3_500n,
      },
    ];
    hooks.liveMessages = [];
    hooks.liveToolCalls = [];
    hooks.hasHydratedCommitted = true;
    hooks.streamStatus = "ready";

    const markup = renderToStaticMarkup(
      createElement(AgentTurnsSection, { sessionId: "session-1" }),
    );

    const pendingIndex = markup.indexOf("pending waitingForApproval");
    const agentIndex = markup.indexOf("objective started");
    const toolIndex = markup.indexOf("ok shell echo hi");

    expect(pendingIndex).toBeGreaterThanOrEqual(0);
    expect(agentIndex).toBeGreaterThanOrEqual(0);
    expect(toolIndex).toBeGreaterThanOrEqual(0);
    expect(pendingIndex).toBeLessThan(agentIndex);
    expect(agentIndex).toBeLessThan(toolIndex);
    expect(markup).toContain('data-section="agent-turns"');
    expect(markup).toContain('data-line-kind="agent"');
    expect(markup).toContain('data-line-kind="tool_result"');
  });

  it("renders the live overlay while the turn is in flight", () => {
    hooks.committedRows = [
      {
        kind: "pendingState",
        cursor: { sequence: 1n },
        sessionId: "session-1",
        runId: "run-1",
        turnId: null,
        occurredAtMs: 1_000n,
        state: "queued",
      },
    ];
    hooks.liveMessages = [
      {
        completed: true,
        firstSequence: 2n,
        lastSequence: 2n,
        occurredAtMs: 2_000n,
        runId: "run-1",
        startedAtMs: 2_000n,
        text: "hello from delta",
        turnId: "turn-1",
      },
    ];
    hooks.liveToolCalls = [
      {
        firstSequence: 3n,
        itemId: "item-1",
        lastSequence: 3n,
        occurredAtMs: 3_000n,
        outcome: "completed",
        output: "echo hi",
        runId: "run-1",
        startedAtMs: 3_000n,
        toolName: "shell",
        turnId: "turn-1",
      },
    ];
    hooks.hasHydratedCommitted = true;
    hooks.streamStatus = "ready";

    const markup = renderToStaticMarkup(
      createElement(AgentTurnsSection, { sessionId: "session-1" }),
    );

    expect(markup).toContain("hello from delta");
    expect(markup).toContain("ok shell echo hi");
  });

  it("renders a completed placeholder when the live store only has a terminal assistant frame", () => {
    hooks.committedRows = [];
    hooks.liveMessages = [
      {
        completed: true,
        firstSequence: 9n,
        lastSequence: 9n,
        occurredAtMs: 2_400n,
        runId: "run-1",
        startedAtMs: 2_400n,
        text: "",
        turnId: "turn-1",
      },
    ];
    hooks.liveToolCalls = [];
    hooks.hasHydratedCommitted = true;
    hooks.streamStatus = "ready";

    const markup = renderToStaticMarkup(
      createElement(AgentTurnsSection, { sessionId: "session-1" }),
    );

    expect(markup).toContain("&gt; agent:      ...");
  });

  it("renders an in-progress assistant row immediately on turn start and fills it once delta arrives", () => {
    hooks.committedRows = [];
    hooks.liveMessages = [
      {
        completed: false,
        firstSequence: 8n,
        lastSequence: 8n,
        occurredAtMs: 2_300n,
        runId: "run-1",
        startedAtMs: 2_300n,
        text: "",
        turnId: "turn-1",
      },
    ];
    hooks.liveToolCalls = [];
    hooks.hasHydratedCommitted = true;
    hooks.streamStatus = "ready";

    const startedMarkup = renderToStaticMarkup(
      createElement(AgentTurnsSection, { sessionId: "session-1" }),
    );

    expect(startedMarkup).toContain("&gt; agent:      thinking...");

    hooks.liveMessages = [
      {
        completed: false,
        firstSequence: 8n,
        lastSequence: 9n,
        occurredAtMs: 2_400n,
        runId: "run-1",
        startedAtMs: 2_300n,
        text: "streaming delivers output incrementally",
        turnId: "turn-1",
      },
    ];

    const deltaMarkup = renderToStaticMarkup(
      createElement(AgentTurnsSection, { sessionId: "session-1" }),
    );

    expect(deltaMarkup).toContain("streaming delivers output incrementally");
    expect(deltaMarkup).not.toContain("thinking...");
  });

  it("drops the live overlay once a committed row exists for the same logical entity", () => {
    hooks.committedRows = [
      {
        kind: "assistant",
        cursor: { sequence: 9n },
        sessionId: "session-1",
        runId: "run-1",
        turnId: "turn-1",
        startedAtMs: 2_000n,
        completedAtMs: 2_400n,
        text: "hello from delta",
      },
    ];
    hooks.liveMessages = [
      {
        completed: true,
        firstSequence: 2n,
        lastSequence: 3n,
        occurredAtMs: 2_400n,
        runId: "run-1",
        startedAtMs: 2_000n,
        text: "hello from delta",
        turnId: "turn-1",
      },
    ];
    hooks.liveToolCalls = [];
    hooks.hasHydratedCommitted = true;
    hooks.streamStatus = "ready";

    const markup = renderToStaticMarkup(
      createElement(AgentTurnsSection, { sessionId: "session-1" }),
    );

    expect(markup.match(/hello from delta/g)).toHaveLength(1);
  });
});
