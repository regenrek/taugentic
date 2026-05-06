import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

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

vi.mock("../../packages/renderer/src/lib/queries/session-queries.js", () => ({
  DEFAULT_ACTIVITY_PAGE_LIMIT: 100,
  DEFAULT_AGENT_TURNS_PAGE_LIMIT: 100,
  SESSION_QUERY_POLL_INTERVAL_MS: 2000,
  useSessionOverviewQuery: () => makeQueryView({ sessions: [] }),
  useSessionRunsQuery: () => makeQueryView([]),
  useSessionAgentTurnsPageQuery: () => makeQueryView([]),
  useSessionActivityPageQuery: () =>
    makeQueryView({
      items: [],
      nextBefore: null,
      latestActivityCursor: null,
    }),
  useSessionApprovalsQuery: () => makeQueryView([]),
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
  useOpenSessionMutation: () => noopMutation,
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

const cortexCalls = vi.hoisted(() => ({
  pausedProps: [] as Array<boolean | undefined>,
}));

vi.mock("../../packages/renderer/src/features/cortex-canvas/index.js", () => ({
  CortexField: (props: { paused?: boolean }) => {
    cortexCalls.pausedProps.push(props.paused);
    return createElement(
      "div",
      { "data-probe": "cortex-field", "data-paused": String(Boolean(props.paused)) },
      "cortex-probe",
    );
  },
  phosphorDecayClass: () => "mc-phosphor-decay",
}));

const { AgentVisualizationPanel } =
  await import("../../packages/renderer/src/features/agent-visualization/index.js");
const motionModule =
  await import("../../packages/renderer/src/features/agent-visualization/state/motion.store.js");

beforeEach(() => {
  cortexCalls.pausedProps = [];
  motionModule.setMotionPaused(false);
});

afterEach(() => {
  motionModule.setMotionPaused(false);
});

describe("AgentVisualizationPanel pause toggle", () => {
  it("forwards paused=false from the motion store to CortexField by default", () => {
    const markup = renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: "session-focused",
        onRunStarted: vi.fn(),
      }),
    );

    expect(motionModule.getMotionPaused()).toBe(false);
    expect(cortexCalls.pausedProps.at(-1)).toBe(false);
    expect(markup).toContain('data-paused="false"');
    expect(markup).toContain("PAUSE");
  });

  it("propagates a true paused flag after toggleMotionPaused() to CortexField", () => {
    motionModule.toggleMotionPaused();
    expect(motionModule.getMotionPaused()).toBe(true);

    const markup = renderToStaticMarkup(
      createElement(AgentVisualizationPanel, {
        sessionId: "session-focused",
        onRunStarted: vi.fn(),
      }),
    );

    expect(cortexCalls.pausedProps.at(-1)).toBe(true);
    expect(markup).toContain('data-paused="true"');
    expect(markup).toContain("RESUME");
  });

  it("setMotionPaused mutates the store deterministically and round-trips via getMotionPaused", () => {
    expect(motionModule.getMotionPaused()).toBe(false);
    motionModule.setMotionPaused(true);
    expect(motionModule.getMotionPaused()).toBe(true);
    motionModule.setMotionPaused(false);
    expect(motionModule.getMotionPaused()).toBe(false);
  });

  it("toggleMotionPaused flips paused on every call", () => {
    expect(motionModule.getMotionPaused()).toBe(false);
    motionModule.toggleMotionPaused();
    expect(motionModule.getMotionPaused()).toBe(true);
    motionModule.toggleMotionPaused();
    expect(motionModule.getMotionPaused()).toBe(false);
  });
});
