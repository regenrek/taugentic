import { afterEach, describe, expect, it, vi } from "vite-plus/test";

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

const noopMutation = {
  mutate: () => {},
  mutateAsync: async () => ({}),
  isPending: false,
  isError: false,
  isSuccess: false,
  status: "idle",
  reset: () => {},
  error: null,
  data: undefined,
};

const queryFixtures = vi.hoisted(() => ({
  approvals: [] as Array<{
    id: string;
    reason: string;
    runId: string;
    scope: "processExec";
  }>,
}));

vi.mock("../../packages/renderer/src/lib/queries/session-queries.js", () => ({
  DEFAULT_ACTIVITY_PAGE_LIMIT: 100,
  DEFAULT_AGENT_TURNS_PAGE_LIMIT: 100,
  SESSION_QUERY_POLL_INTERVAL_MS: 2000,
  useSessionOverviewQuery: () => makeQueryView({ sessions: [] }),
  useSessionRunsQuery: () =>
    makeQueryView([
      {
        id: "run-1",
        runtimeProfileId: "profile-1",
        objective: "first objective",
        status: "running",
      },
    ]),
  useSessionActivityPageQuery: () =>
    makeQueryView({
      items: [],
      nextBefore: null,
      latestActivityCursor: null,
    }),
  useSessionAgentTurnsPageQuery: () => makeQueryView([]),
  useSessionApprovalsQuery: () => makeQueryView(queryFixtures.approvals),
  useSessionArtifactsQuery: () => makeQueryView([]),
}));

vi.mock("../../packages/renderer/src/features/agent-stream/index.js", () => ({
  useAgentStream: () => ({
    committedRows: [],
    errorMessage: null,
    hasHydratedCommitted: true,
    liveMessages: [],
    liveToolCalls: [],
    streamStatus: "ready",
  }),
}));

vi.mock("../../packages/renderer/src/lib/queries/session-mutations.js", () => ({
  useStartRunMutation: () => noopMutation,
  useDecideApprovalMutation: () => noopMutation,
}));

vi.mock("../../packages/renderer/src/lib/queries/recipes.js", () => ({
  useRecipesQuery: () => makeQueryView([]),
}));

vi.mock(
  "../../packages/renderer/src/features/session-detail/useSessionApprovalLiveSync.js",
  () => ({
    useSessionApprovalLiveSync: () => ({
      errorMessage: null,
      lastSequence: null,
      streamStatus: "ready",
    }),
  }),
);

vi.mock("../../packages/renderer/src/features/cortex-canvas/index.js", () => ({
  CortexField: () => createElement("div", { "data-probe": "cortex-field" }, "cortex-probe"),
  phosphorDecayClass: () => "mc-phosphor-decay",
}));

const { AgentVisualizationPanel } =
  await import("../../packages/renderer/src/features/agent-visualization/index.js");

describe("AgentVisualizationPanel focused state", () => {
  afterEach(() => {
    queryFixtures.approvals = [];
  });

  it("renders RunHeader, CortexField, and FocusedRunTabs in order", () => {
    const markup = renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: "session-focused",
        onRunStarted: vi.fn(),
      }),
    );

    expect(markup).toContain('data-agent-visualization="focused"');
    expect(markup).toContain('data-session-id="session-focused"');

    expect(markup).toContain("data-agent-visualization-run-header");
    expect(markup).toContain('data-probe="cortex-field"');
    expect(markup).toContain("data-agent-visualization-tabs");

    const headerIdx = markup.indexOf("data-agent-visualization-run-header");
    const cortexIdx = markup.indexOf('data-probe="cortex-field"');
    const tabsIdx = markup.indexOf("data-agent-visualization-tabs");

    expect(headerIdx).toBeGreaterThan(-1);
    expect(cortexIdx).toBeGreaterThan(headerIdx);
    expect(tabsIdx).toBeGreaterThan(cortexIdx);
  });

  it("renders all five tab triggers with the documented labels", () => {
    const markup = renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: "session-focused",
        onRunStarted: vi.fn(),
      }),
    );

    expect(markup).toContain(">Steps<");
    expect(markup).toContain(">Tool Calls<");
    expect(markup).toContain(">Diff<");
    expect(markup).toContain(">Metrics<");
    expect(markup).toContain(">Raw<");
  });

  it("defaults the focused tabs to the steps panel", () => {
    const markup = renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: "session-focused",
        onRunStarted: vi.fn(),
      }),
    );

    const ariaSelectedTrueMatches = markup.match(/aria-selected="true"/g) ?? [];
    expect(ariaSelectedTrueMatches.length).toBe(1);

    const stepsLabelIdx = markup.indexOf(">Steps<");
    expect(stepsLabelIdx).toBeGreaterThan(-1);
    const trueIdx = markup.indexOf('aria-selected="true"');
    expect(trueIdx).toBeGreaterThan(-1);

    // The single aria-selected="true" attribute must belong to the Steps
    // trigger button: it appears earlier in the markup than the literal
    // ">Steps<" label, AND no other tab trigger label sits between them.
    expect(trueIdx).toBeLessThan(stepsLabelIdx);
    const between = markup.slice(trueIdx, stepsLabelIdx);
    expect(between).not.toContain(">Tool Calls<");
    expect(between).not.toContain(">Diff<");
    expect(between).not.toContain(">Metrics<");
    expect(between).not.toContain(">Raw<");
  });

  it("surfaces the latest run status from useSessionRunsQuery in RunHeader", () => {
    const markup = renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: "session-focused",
        onRunStarted: vi.fn(),
      }),
    );

    expect(markup).toContain("first objective");
    expect(markup).toContain("running");
    expect(markup).toContain("PAUSE");
  });

  it("surfaces pending approval actions in the default center tab", () => {
    queryFixtures.approvals = [
      {
        id: "approval-1",
        reason: "Allow Codex ACP safe command",
        runId: "run-1",
        scope: "processExec",
      },
    ];

    const markup = renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: "session-focused",
        onRunStarted: vi.fn(),
      }),
    );

    expect(markup).toContain('data-agent-visualization-tab="steps"');
    expect(markup).toContain('data-approval-surface="mission-control"');
    expect(markup).toContain("approval required");
    expect(markup).toContain("Allow Codex ACP safe command");
    expect(markup).toContain(">approve<");
    expect(markup).toContain(">reject<");
  });
});
