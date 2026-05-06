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

function makeQueryView<T>(data: T) {
  return {
    data,
    error: null,
    isLoading: false,
    isFetching: false,
    refetch: async () => ({}),
  };
}

vi.mock("../../packages/renderer/src/lib/queries/session-queries.js", () => ({
  DEFAULT_ACTIVITY_PAGE_LIMIT: 100,
  SESSION_QUERY_POLL_INTERVAL_MS: 2000,
  useSessionOverviewQuery: () => makeQueryView({ sessions: [] }),
  useSessionRunsQuery: () => makeQueryView([]),
  useSessionActivityQuery: () => makeQueryView([]),
  useSessionApprovalsQuery: () => makeQueryView([]),
  useSessionArtifactsQuery: () => makeQueryView([]),
}));

vi.mock("../../packages/renderer/src/features/session-detail/index.js", () => ({
  RunsSection: ({ sessionId }: { sessionId: string }) =>
    createElement(
      "div",
      { "data-probe": "wrapped-runs", "data-session-id": sessionId },
      "wrapped-runs-probe",
    ),
  AgentTurnsSection: ({ sessionId }: { sessionId: string }) =>
    createElement(
      "div",
      { "data-probe": "wrapped-agent-turns", "data-session-id": sessionId },
      "wrapped-agent-turns-probe",
    ),
  ApprovalsSection: ({ sessionId }: { sessionId: string }) =>
    createElement(
      "div",
      { "data-probe": "wrapped-approvals", "data-session-id": sessionId },
      "wrapped-approvals-probe",
    ),
  ArtifactsSection: ({ sessionId }: { sessionId: string }) =>
    createElement(
      "div",
      { "data-probe": "wrapped-artifacts", "data-session-id": sessionId },
      "wrapped-artifacts-probe",
    ),
  MetricsSection: ({ sessionId }: { sessionId: string }) =>
    createElement(
      "div",
      { "data-probe": "wrapped-metrics", "data-session-id": sessionId },
      "wrapped-metrics-probe",
    ),
  formatActivityLine: () => ({
    kind: "user" as const,
    key: "k",
    occurredAtMs: 0,
    text: "",
  }),
  sortActivityAscending: <T>(items: T[]) => items,
}));

const { FocusedRunTabs } =
  await import("../../packages/renderer/src/features/agent-visualization/FocusedRunTabs.js");

const TAB_PROBES = {
  steps: "wrapped-agent-turns-probe",
  tools: "wrapped-approvals-probe",
  diff: "wrapped-artifacts-probe",
  metrics: "wrapped-metrics-probe",
  raw: undefined,
} as const;

function renderTabs(value: "steps" | "tools" | "diff" | "metrics" | "raw"): string {
  return renderToStaticMarkup(
    createElement(FocusedRunTabs, {
      sessionId: "session-tabs",
      value,
    }),
  );
}

describe("AgentVisualizationPanel tab switching", () => {
  it("steps tab renders the wrapped AgentTurnsSection and RunsSection children", () => {
    const markup = renderTabs("steps");
    expect(markup).toContain("wrapped-runs-probe");
    expect(markup).toContain("wrapped-approvals-probe");
    expect(markup).toContain("wrapped-agent-turns-probe");
    expect(markup).toContain(TAB_PROBES.steps);
    expect(markup).toContain('data-agent-visualization-tab="steps"');
  });

  it("tools tab renders the wrapped ApprovalsSection child", () => {
    const markup = renderTabs("tools");
    expect(markup).toContain(TAB_PROBES.tools);
    expect(markup).toContain('data-agent-visualization-tab="tools"');
  });

  it("diff tab renders the wrapped ArtifactsSection child", () => {
    const markup = renderTabs("diff");
    expect(markup).toContain(TAB_PROBES.diff);
    expect(markup).toContain('data-agent-visualization-tab="diff"');
  });

  it("metrics tab renders the wrapped MetricsSection child", () => {
    const markup = renderTabs("metrics");
    expect(markup).toContain(TAB_PROBES.metrics);
    expect(markup).toContain('data-agent-visualization-tab="metrics"');
  });

  it("raw tab renders the raw events region (no SessionDetail child wrapped)", () => {
    const markup = renderTabs("raw");
    expect(markup).toContain('data-agent-visualization-tab="raw"');
    expect(markup).toContain("raw events");
  });

  it("emits triggers for all five tabs regardless of which value is active", () => {
    const markup = renderTabs("steps");
    expect(markup).toContain(">Steps<");
    expect(markup).toContain(">Tool Calls<");
    expect(markup).toContain(">Diff<");
    expect(markup).toContain(">Metrics<");
    expect(markup).toContain(">Raw<");
  });

  it("propagates the focused sessionId into every wrapped child as data-session-id", () => {
    const markup = renderTabs("steps");
    expect(markup).toContain('data-session-id="session-tabs"');
  });
});
