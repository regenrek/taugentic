import { describe, expect, it } from "vite-plus/test";

import type { SessionOverview } from "../../packages/shared/generated/index.js";
import { RailHeaderCountsView } from "../../packages/renderer/src/features/overview/index.js";
import { aggregateLaneCounts } from "../../packages/renderer/src/features/overview/formatters.js";

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

function makeOverview(overrides: Partial<SessionOverview>): SessionOverview {
  return {
    approvalAttention: "idle",
    isActive: false,
    laneStatus: "idle",
    pendingApprovalCount: 0,
    session: {
      id: overrides.session?.id ?? "session-x",
      status: "running",
      title: overrides.session?.title ?? "Session X",
    },
    ...overrides,
  };
}

function readChipCount(markup: string, label: string): number {
  const chipMatch = markup.match(
    new RegExp(`data-rail-count-chip="${label}"[\\s\\S]*?tabular-nums[^>]*>([0-9]+)<`),
  );
  if (chipMatch === null) {
    throw new Error(`chip "${label}" not found`);
  }
  return Number.parseInt(chipMatch[1] ?? "", 10);
}

describe("RailHeaderCountsView", () => {
  it("renders the three canonical counts derived from the fixture overview", () => {
    const sessions: SessionOverview[] = [
      makeOverview({ laneStatus: "active", session: { id: "s1", status: "running", title: "" } }),
      makeOverview({ laneStatus: "active", session: { id: "s2", status: "running", title: "" } }),
      makeOverview({
        laneStatus: "waitingForApproval",
        pendingApprovalCount: 1,
        session: { id: "s3", status: "paused", title: "" },
      }),
      makeOverview({ laneStatus: "failed", session: { id: "s4", status: "failed", title: "" } }),
      makeOverview({ laneStatus: "failed", session: { id: "s5", status: "failed", title: "" } }),
      makeOverview({ laneStatus: "failed", session: { id: "s6", status: "failed", title: "" } }),
      makeOverview({ laneStatus: "idle", session: { id: "s7", status: "running", title: "" } }),
      makeOverview({
        laneStatus: "completed",
        session: { id: "s8", status: "completed", title: "" },
      }),
      makeOverview({
        laneStatus: "cancelled",
        session: { id: "s9", status: "failed", title: "" },
      }),
    ];
    const counts = aggregateLaneCounts(sessions);

    const markup = renderToStaticMarkup(createElement(RailHeaderCountsView, { counts }));

    expect(markup).toContain('data-feature="rail-header-counts"');
    expect(markup).toContain('data-rail-count-chip="active"');
    expect(markup).toContain('data-rail-count-chip="wait"');
    expect(markup).toContain('data-rail-count-chip="failed"');
    expect(markup).toContain('aria-label="Session lane counts"');

    expect(readChipCount(markup, "active")).toBe(2);
    expect(readChipCount(markup, "wait")).toBe(1);
    expect(readChipCount(markup, "failed")).toBe(3);
  });

  it("renders zeros when there are no sessions", () => {
    const counts = aggregateLaneCounts([]);
    const markup = renderToStaticMarkup(createElement(RailHeaderCountsView, { counts }));

    expect(readChipCount(markup, "active")).toBe(0);
    expect(readChipCount(markup, "wait")).toBe(0);
    expect(readChipCount(markup, "failed")).toBe(0);
  });
});
