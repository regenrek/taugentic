import { describe, expect, it, vi } from "vite-plus/test";

import { ApprovalsPanel } from "../../packages/renderer/src/features/approvals/index.js";
import { ArtifactsPanel } from "../../packages/renderer/src/features/artifacts/index.js";
import { RunsPanel, RunStatusBadge } from "../../packages/renderer/src/features/runs/index.js";
import {
  getRunStatusPresentation,
  type RunPresentationStatus,
} from "../../packages/renderer/src/features/run-status/presentation.js";
import { SessionsPanel } from "../../packages/renderer/src/features/sessions/index.js";
import { createTestQueryClient, withQueryClient } from "./support/with-query-client.js";

type CreateElementFn = (
  component: (...args: any[]) => unknown,
  props?: Record<string, unknown> | null,
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

vi.mock("../../packages/renderer/src/lib/queries/session-queries.js", () => ({
  DEFAULT_AGENT_TURNS_PAGE_LIMIT: 100,
  useSessionsQuery: () => ({
    data: [],
    error: null,
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
  }),
  useSessionActivityQuery: () => ({
    data: [],
    error: null,
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
  }),
  useSessionApprovalsQuery: () => ({
    data: [],
    error: null,
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
  }),
  useSessionArtifactsQuery: () => ({
    data: [],
    error: null,
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
  }),
  useSessionRunsQuery: () => ({
    data: [],
    error: null,
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
  }),
  useSessionAgentTurnsPageQuery: () => ({
    data: [],
    error: null,
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
  }),
  useSessionRunActivityQuery: () => ({
    data: [],
    error: null,
    isLoading: false,
    isFetching: false,
    refetch: vi.fn(),
  }),
}));

function renderSessionsPanel(currentSessionId: string | null = null): string {
  return renderToStaticMarkup(
    withQueryClient(
      createTestQueryClient(),
      createElement(SessionsPanel, { currentSessionId, onSessionChange: () => {} }),
    ),
  );
}

function renderRunsPanel(sessionId: string): string {
  return renderToStaticMarkup(
    withQueryClient(createTestQueryClient(), createElement(RunsPanel, { sessionId })),
  );
}

function renderApprovalsPanel(sessionId: string): string {
  return renderToStaticMarkup(
    withQueryClient(createTestQueryClient(), createElement(ApprovalsPanel, { sessionId })),
  );
}

function renderArtifactsPanel(sessionId: string): string {
  return renderToStaticMarkup(
    withQueryClient(createTestQueryClient(), createElement(ArtifactsPanel, { sessionId })),
  );
}

describe("renderer session surfaces", () => {
  it("shows sessions copy without architecture wording", () => {
    const markup = renderSessionsPanel();

    expect(markup).toContain(
      "Open a coding workspace, keep the active session pinned, and reuse it across runs and review surfaces.",
    );
    expect(markup).toContain("No sessions yet. Open your first session here.");
    expect(markup).not.toContain("Renderer owns only the selected session id.");
  });

  it("binds run status copy only to the explicit current session id", () => {
    const markup = renderRunsPanel("session-42");

    expect(markup).toContain("session-42");
    expect(markup).toContain("Active session");
    expect(markup).toContain("Loading runs for the selected session...");
    expect(markup).toContain("Waiting for run activity...");
    expect(markup).toContain("Start run");
  });

  it("renders runs status badges with canonical presentation", () => {
    const cases = [
      "completed",
      "failed",
      "running",
      "quarantined",
      "contractViolation",
    ] as const satisfies readonly RunPresentationStatus[];

    for (const status of cases) {
      const presentation = getRunStatusPresentation(status);
      const markup = renderToStaticMarkup(createElement(RunStatusBadge, { status }));

      expect(markup).toContain(presentation.label);
      expect(markup).toContain(`data-variant="${presentation.badgeVariant}"`);
    }
  });

  it("binds approval copy only to the explicit current session id", () => {
    const markup = renderApprovalsPanel("session-42");

    expect(markup).toContain("Session");
    expect(markup).toContain("session-42");
    expect(markup).toContain("Loading approvals for this session...");
  });

  it("binds artifact copy only to the explicit current session id", () => {
    const markup = renderArtifactsPanel("session-42");

    expect(markup).toContain("Session");
    expect(markup).toContain("session-42");
    expect(markup).toContain("Loading artifacts for the selected session...");
  });
});
