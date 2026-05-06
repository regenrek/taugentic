import { describe, expect, it } from "vite-plus/test";

import type { RunTimeline } from "../../packages/shared/src/contracts.js";
import {
  createTimelineLanes,
  formatTimelineEvent,
  formatTimelineRange,
  shortRunId,
} from "../../packages/renderer/src/features/run-tree/timeline-model.js";

function timeline(): RunTimeline {
  return {
    sessionId: "session-1",
    rootRunId: "run-parent",
    latestEventSeq: 3n,
    runs: [
      {
        runId: "run-parent",
        depth: 0,
        status: "running",
        startedAtMs: 1_000n,
      },
      {
        runId: "run-child",
        parentRunId: "run-parent",
        depth: 1,
        status: "completed",
        startedAtMs: 2_000n,
        endedAtMs: 4_000n,
      },
    ],
    events: [
      {
        seq: 1n,
        occurredAtMs: 1_000n,
        runId: "run-parent",
        kind: "runStatus",
        status: "running",
        label: "parent running",
        payload: { kind: "run", detail: "parent running" },
      },
      {
        seq: 2n,
        occurredAtMs: 2_000n,
        runId: "run-child",
        kind: "claimConflict",
        label: "claim conflict warning",
        payload: {
          kind: "conflict",
          warning: {
            requestingCapsule: "run-child",
            severity: "warning",
            conflicts: [
              {
                file: "apps/desktop/package.json",
                holdingCapsule: "run-parent",
                holdingKind: "write",
              },
            ],
          },
        },
      },
    ],
  };
}

describe("run timeline model", () => {
  it("groups events into run lanes without mirroring server state", () => {
    const lanes = createTimelineLanes(timeline());

    expect(lanes.map((lane) => [lane.run.runId, lane.events.map((event) => event.seq)])).toEqual([
      ["run-parent", [1n]],
      ["run-child", [2n]],
    ]);
  });

  it("formats lane ranges and event labels", () => {
    const lanes = createTimelineLanes(timeline());

    expect(formatTimelineRange(lanes[0]!.run)).toContain("started");
    expect(formatTimelineRange(lanes[1]!.run)).toContain(" - ");
    expect(formatTimelineEvent(lanes[1]!.events[0]!)).toContain("claimConflict");
    expect(shortRunId("run-short")).toBe("run-short");
  });
});
