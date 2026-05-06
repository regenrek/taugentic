import type { SessionOverview, SessionOverviewLaneStatus } from "@taugentic/desktop-shared";

export type OverviewLaneTone = "idle" | "active" | "waiting" | "failed" | "completed" | "cancelled";

export interface OverviewLanePresentation {
  label: string;
  tone: OverviewLaneTone;
}

export function describeLaneStatus(status: SessionOverviewLaneStatus): OverviewLanePresentation {
  switch (status) {
    case "active":
      return { label: "Active", tone: "active" };
    case "waitingForApproval":
      return { label: "Waiting for approval", tone: "waiting" };
    case "failed":
      return { label: "Failed", tone: "failed" };
    case "completed":
      return { label: "Completed", tone: "completed" };
    case "cancelled":
      return { label: "Cancelled", tone: "cancelled" };
    case "idle":
    default:
      return { label: "Idle", tone: "idle" };
  }
}

export interface OverviewLaneCounts {
  active: number;
  cancelled: number;
  completed: number;
  failed: number;
  idle: number;
  total: number;
  waiting: number;
}

export function aggregateLaneCounts(sessions: SessionOverview[]): OverviewLaneCounts {
  const counts: OverviewLaneCounts = {
    active: 0,
    cancelled: 0,
    completed: 0,
    failed: 0,
    idle: 0,
    total: sessions.length,
    waiting: 0,
  };
  for (const session of sessions) {
    switch (session.laneStatus) {
      case "active":
        counts.active += 1;
        break;
      case "waitingForApproval":
        counts.waiting += 1;
        break;
      case "failed":
        counts.failed += 1;
        break;
      case "completed":
        counts.completed += 1;
        break;
      case "cancelled":
        counts.cancelled += 1;
        break;
      case "idle":
      default:
        counts.idle += 1;
        break;
    }
  }
  return counts;
}

const LANE_STATUS_SORT_ORDER: Record<SessionOverviewLaneStatus, number> = {
  waitingForApproval: 0,
  failed: 1,
  active: 2,
  idle: 3,
  completed: 4,
  cancelled: 5,
};

export function sortSessionsForOperator(sessions: SessionOverview[]): SessionOverview[] {
  return [...sessions].sort((left, right) => {
    const laneDelta =
      LANE_STATUS_SORT_ORDER[left.laneStatus] - LANE_STATUS_SORT_ORDER[right.laneStatus];
    if (laneDelta !== 0) {
      return laneDelta;
    }
    const leftActivity = left.lastActivityAtMs ?? 0n;
    const rightActivity = right.lastActivityAtMs ?? 0n;
    if (leftActivity === rightActivity) {
      return left.session.id.localeCompare(right.session.id);
    }
    return rightActivity > leftActivity ? 1 : -1;
  });
}

export function formatRelativeTimeMs(
  valueMs: bigint | number | null | undefined,
  nowMs: number,
): string {
  if (valueMs === null || valueMs === undefined) {
    return "never";
  }
  const asNumber = typeof valueMs === "bigint" ? Number(valueMs) : valueMs;
  const deltaMs = Math.max(0, nowMs - asNumber);
  if (deltaMs < 5_000) {
    return "just now";
  }
  if (deltaMs < 60_000) {
    return `${Math.floor(deltaMs / 1000)}s ago`;
  }
  if (deltaMs < 3_600_000) {
    return `${Math.floor(deltaMs / 60_000)}m ago`;
  }
  if (deltaMs < 86_400_000) {
    return `${Math.floor(deltaMs / 3_600_000)}h ago`;
  }
  return `${Math.floor(deltaMs / 86_400_000)}d ago`;
}
