import { describe, expect, it, vi } from "vite-plus/test";

import type { RunListEntry } from "../../packages/shared/src/contracts.js";
import { RunReplayControl } from "../../packages/renderer/src/features/run-tree/RunReplayControl.js";

type CreateElementFn = (
  component: unknown,
  props?: Record<string, unknown> | null,
  ...children: unknown[]
) => unknown;
type RenderToStaticMarkupFn = (element: unknown) => string;

const reactModulePath = "../../packages/renderer/node_modules/react/index.js";
const reactServerModulePath = "../../packages/renderer/node_modules/react-dom/server.node.js";
const queryModulePath =
  "../../packages/renderer/node_modules/@tanstack/react-query/build/modern/index.js";

const { createElement } = (await import(reactModulePath)) as {
  createElement: CreateElementFn;
};
const { renderToStaticMarkup } = (await import(reactServerModulePath)) as {
  renderToStaticMarkup: RenderToStaticMarkupFn;
};
const { QueryClient, QueryClientProvider } = (await import(queryModulePath)) as {
  QueryClient: new () => unknown;
  QueryClientProvider: unknown;
};

vi.mock("../../packages/renderer/src/lib/ipc/api.js", () => ({
  forkRun: vi.fn(),
}));

function makeRun(overrides: Partial<RunListEntry> = {}): RunListEntry {
  return {
    harness: "native",
    id: "run-replay-123",
    lastEventSeq: 42n,
    objectivePreview: "Replay this run",
    outputContract: "patch",
    recipeId: null,
    status: "completed",
    ...overrides,
  };
}

function renderReplay(run: RunListEntry, sessionId: string | null = "session-1"): string {
  return renderToStaticMarkup(
    createElement(
      QueryClientProvider,
      { client: new QueryClient() },
      createElement(RunReplayControl, { run, sessionId }),
    ),
  );
}

describe("RunReplayControl", () => {
  it("renders replay affordance for terminal runs with a fork point", () => {
    const markup = renderReplay(makeRun());

    expect(markup).toContain("Replay");
  });

  it("does not render before a terminal fork point exists", () => {
    const markup = renderReplay(makeRun({ lastEventSeq: null, status: "running" }));

    expect(markup).toBe("");
  });
});
