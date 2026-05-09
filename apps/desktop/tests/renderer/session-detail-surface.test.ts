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
  useSessionNativeRunsQuery: () => makeQueryView({ nextCursor: null, runs: [] }),
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
      streamStatus: "ready",
      errorMessage: null,
      lastSequence: null,
    }),
  }),
);

vi.mock("../../packages/renderer/src/features/session-detail/useArtifactContent.js", () => ({
  useArtifactContentQuery: () => ({
    data: undefined,
    error: null,
    isPending: false,
    isFetching: false,
  }),
  useSaveArtifactAsMutation: () => ({
    isPending: false,
    variables: undefined,
    mutate: () => {},
    mutateAsync: async () => ({}),
  }),
  defaultArtifactFilename: () => "artifact.diff",
}));

const { SessionDetailSurface } =
  await import("../../packages/renderer/src/features/session-detail/SessionDetailSurface.js");

describe("SessionDetailSurface", () => {
  it("renders a terse empty state when sessionId is null", () => {
    const markup = renderToStaticMarkup(createElement(SessionDetailSurface, { sessionId: null }));

    expect(markup).toContain('data-session-detail="empty"');
    expect(markup).toContain("Select a session in the left rail");
    expect(markup).not.toContain('data-section="runs"');
  });

  it("renders all inline sections with their headers when a session is selected", () => {
    const markup = renderToStaticMarkup(
      createElement(SessionDetailSurface, { sessionId: "session-42" }),
    );

    expect(markup).toContain('data-session-detail="bound"');
    expect(markup).toContain('data-session-id="session-42"');

    expect(markup).toContain('data-section="runs"');
    expect(markup).toContain('data-section-header="runs"');
    expect(markup).toContain('data-section="run-tree"');
    expect(markup).toContain('data-section-header="run tree"');
    expect(markup).toContain('data-section="agent-turns"');
    expect(markup).toContain('data-section-header="agent turns"');
    expect(markup).toContain('data-section="approval-inbox"');
    expect(markup).toContain("approval inbox");
    expect(markup).toContain('data-section="artifacts"');
    expect(markup).toContain('data-section-header="artifacts"');
  });
});
