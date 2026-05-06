import { describe, expect, it } from "vite-plus/test";

import type { SessionOverviewLaneStatus } from "../../packages/shared/generated/index.js";
import { LaneStatusBadge } from "../../packages/renderer/src/features/overview/index.js";

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

const STATUS_TONE_MATRIX: ReadonlyArray<{
  expectedLabel: string;
  expectedTone: string;
  laneStatus: SessionOverviewLaneStatus;
}> = [
  { expectedLabel: "Active", expectedTone: "active", laneStatus: "active" },
  {
    expectedLabel: "Waiting for approval",
    expectedTone: "waiting",
    laneStatus: "waitingForApproval",
  },
  { expectedLabel: "Failed", expectedTone: "failed", laneStatus: "failed" },
  { expectedLabel: "Completed", expectedTone: "completed", laneStatus: "completed" },
  { expectedLabel: "Cancelled", expectedTone: "cancelled", laneStatus: "cancelled" },
  { expectedLabel: "Idle", expectedTone: "idle", laneStatus: "idle" },
];

describe("LaneStatusBadge", () => {
  for (const { expectedLabel, expectedTone, laneStatus } of STATUS_TONE_MATRIX) {
    it(`renders the canonical tone, label, and ARIA for laneStatus=${laneStatus}`, () => {
      const markup = renderToStaticMarkup(createElement(LaneStatusBadge, { laneStatus }));

      expect(markup).toContain(`data-lane-status="${laneStatus}"`);
      expect(markup).toContain(`data-tone="${expectedTone}"`);
      expect(markup).toContain(`aria-label="Lane status: ${expectedLabel}"`);
      expect(markup).toContain(`role="status"`);
      expect(markup).toContain(`var(--status-${expectedTone})`);
      expect(markup).toContain(expectedLabel);
    });
  }

  it("hides the textual label when showLabel=false", () => {
    const markup = renderToStaticMarkup(
      createElement(LaneStatusBadge, { laneStatus: "active", showLabel: false }),
    );

    expect(markup).toContain('data-lane-status="active"');
    expect(markup).not.toContain("data-lane-status-label");
    expect(markup).not.toContain(">Active<");
  });
});
