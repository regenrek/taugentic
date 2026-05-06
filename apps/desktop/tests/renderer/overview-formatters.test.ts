import { describe, expect, it } from "vite-plus/test";

import type { SessionOverview } from "../../packages/shared/generated/index.js";
import {
  aggregateLaneCounts,
  describeLaneStatus,
  formatRelativeTimeMs,
  sortSessionsForOperator,
} from "../../packages/renderer/src/features/overview/formatters.js";

function makeOverview(overrides: Partial<SessionOverview> = {}): SessionOverview {
  return {
    approvalAttention: "idle",
    isActive: false,
    laneStatus: "idle",
    pendingApprovalCount: 0,
    session: {
      id: overrides.session?.id ?? "session-x",
      status: "idle",
      title: overrides.session?.title ?? "Untitled",
    },
    ...overrides,
  };
}

describe("overview formatters", () => {
  it("maps canonical lane status values to presentation labels", () => {
    expect(describeLaneStatus("idle").label).toBe("Idle");
    expect(describeLaneStatus("active").label).toBe("Active");
    expect(describeLaneStatus("waitingForApproval").label).toBe("Waiting for approval");
    expect(describeLaneStatus("failed").tone).toBe("failed");
    expect(describeLaneStatus("completed").tone).toBe("completed");
    expect(describeLaneStatus("cancelled").tone).toBe("cancelled");
  });

  it("aggregates lane counts across every canonical status", () => {
    const sessions: SessionOverview[] = [
      makeOverview({ laneStatus: "active", session: { id: "s-1", status: "running", title: "A" } }),
      makeOverview({
        laneStatus: "waitingForApproval",
        session: { id: "s-2", status: "paused", title: "B" },
      }),
      makeOverview({
        laneStatus: "waitingForApproval",
        session: { id: "s-3", status: "paused", title: "C" },
      }),
      makeOverview({ laneStatus: "failed", session: { id: "s-4", status: "failed", title: "D" } }),
      makeOverview({ laneStatus: "idle", session: { id: "s-5", status: "idle", title: "E" } }),
      makeOverview({
        laneStatus: "completed",
        session: { id: "s-6", status: "completed", title: "F" },
      }),
      makeOverview({
        laneStatus: "cancelled",
        session: { id: "s-7", status: "completed", title: "G" },
      }),
    ];

    expect(aggregateLaneCounts(sessions)).toEqual({
      active: 1,
      cancelled: 1,
      completed: 1,
      failed: 1,
      idle: 1,
      total: 7,
      waiting: 2,
    });
  });

  it("sorts sessions so attention-requiring lanes come first and most recent activity wins ties", () => {
    const sessions: SessionOverview[] = [
      makeOverview({
        laneStatus: "idle",
        lastActivityAtMs: 100n,
        session: { id: "s-idle", status: "idle", title: "Idle" },
      }),
      makeOverview({
        laneStatus: "active",
        lastActivityAtMs: 500n,
        session: { id: "s-active-old", status: "running", title: "Active old" },
      }),
      makeOverview({
        laneStatus: "active",
        lastActivityAtMs: 900n,
        session: { id: "s-active-new", status: "running", title: "Active new" },
      }),
      makeOverview({
        laneStatus: "waitingForApproval",
        lastActivityAtMs: 400n,
        session: { id: "s-waiting", status: "paused", title: "Waiting" },
      }),
      makeOverview({
        laneStatus: "failed",
        lastActivityAtMs: 300n,
        session: { id: "s-failed", status: "failed", title: "Failed" },
      }),
    ];

    const sorted = sortSessionsForOperator(sessions).map((overview) => overview.session.id);

    expect(sorted).toEqual(["s-waiting", "s-failed", "s-active-new", "s-active-old", "s-idle"]);
  });

  it("formats relative timestamps for the operator badge", () => {
    const now = 10_000;
    expect(formatRelativeTimeMs(null, now)).toBe("never");
    expect(formatRelativeTimeMs(9_000, now)).toBe("just now");
    expect(formatRelativeTimeMs(0, now)).toBe("10s ago");
    expect(formatRelativeTimeMs(now - 120_000, now)).toBe("2m ago");
    expect(formatRelativeTimeMs(now - 7_200_000, now)).toBe("2h ago");
    expect(formatRelativeTimeMs(now - 2 * 86_400_000, now)).toBe("2d ago");
  });
});
