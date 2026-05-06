import { describe, expect, it, vi } from "vite-plus/test";

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

const cortexProbe = vi.hoisted(() => ({ mountCount: 0 }));
const agentStreamProbe = vi.hoisted(() => ({ readCount: 0 }));

vi.mock("../../packages/renderer/src/features/cortex-canvas/index.js", () => ({
  CortexField: () => {
    cortexProbe.mountCount += 1;
    return createElement("div", { "data-probe": "cortex-field" }, "cortex-probe");
  },
  phosphorDecayClass: () => "mc-phosphor-decay",
}));

vi.mock("../../packages/renderer/src/features/agent-stream/index.js", () => ({
  useAgentStream: () => {
    agentStreamProbe.readCount += 1;
    return {
      committedRows: [],
      errorMessage: null,
      hasHydratedCommitted: true,
      liveMessages: [],
      liveToolCalls: [],
      streamStatus: "ready",
    };
  },
}));

const { AgentVisualizationPanel } =
  await import("../../packages/renderer/src/features/agent-visualization/index.js");

describe("AgentVisualizationPanel empty state", () => {
  it("renders the dim placeholder when no session is selected", () => {
    cortexProbe.mountCount = 0;
    agentStreamProbe.readCount = 0;

    const markup = renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: null,
        onRunStarted: vi.fn(),
      }),
    );

    expect(markup).toContain('data-agent-visualization="empty"');
    expect(markup).toContain("Select a session in the left rail");
  });

  it("does not mount CortexField when sessionId is null", () => {
    cortexProbe.mountCount = 0;
    agentStreamProbe.readCount = 0;

    const markup = renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: null,
        onRunStarted: vi.fn(),
      }),
    );

    expect(markup).not.toContain("cortex-probe");
    expect(markup).not.toContain('data-probe="cortex-field"');
    expect(cortexProbe.mountCount).toBe(0);
  });

  it("does not read the agent-stream projection when sessionId is null", () => {
    cortexProbe.mountCount = 0;
    agentStreamProbe.readCount = 0;

    renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: null,
        onRunStarted: vi.fn(),
      }),
    );

    expect(agentStreamProbe.readCount).toBe(0);
  });

  it("does not include the focused-run scaffolding (RunHeader, stream, tabs)", () => {
    const markup = renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: null,
        onRunStarted: vi.fn(),
      }),
    );

    expect(markup).not.toContain("data-agent-visualization-run-header");
    expect(markup).not.toContain("data-agent-visualization-stream");
    expect(markup).not.toContain("data-agent-visualization-tabs");
    expect(markup).not.toContain('data-agent-visualization="focused"');
  });
});
