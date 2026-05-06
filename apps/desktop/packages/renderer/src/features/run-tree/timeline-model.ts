import type { RunTimeline, RunTimelineEvent, RunTimelineRun } from "@taugentic/desktop-shared";

export interface TimelineLane {
  events: RunTimelineEvent[];
  run: RunTimelineRun;
}

export function createTimelineLanes(timeline: RunTimeline | null): TimelineLane[] {
  if (timeline === null) {
    return [];
  }
  const eventsByRun = new Map<string, RunTimelineEvent[]>();
  for (const event of timeline.events) {
    const events = eventsByRun.get(event.runId) ?? [];
    events.push(event);
    eventsByRun.set(event.runId, events);
  }
  return timeline.runs.map((run) => ({
    events: eventsByRun.get(run.runId) ?? [],
    run,
  }));
}

export function formatTimelineRange(run: RunTimelineRun): string {
  const started = toNumber(run.startedAtMs);
  if (started === null) {
    return "not started";
  }
  const ended = toNumber(run.endedAtMs);
  if (ended === null) {
    return `started ${formatClock(started)}`;
  }
  return `${formatClock(started)} - ${formatClock(ended)}`;
}

export function formatTimelineEvent(event: RunTimelineEvent): string {
  const at = formatClock(toNumber(event.occurredAtMs) ?? 0);
  const status = event.status ? ` ${event.status}` : "";
  return `${at} ${event.kind}${status}: ${event.label}`;
}

export function shortRunId(runId: string): string {
  if (runId.length <= 14) {
    return runId;
  }
  return `${runId.slice(0, 8)}...${runId.slice(-4)}`;
}

function formatClock(ms: number): string {
  return new Date(ms).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function toNumber(value: bigint | number | null | undefined): number | null {
  if (value === null || value === undefined) {
    return null;
  }
  return typeof value === "bigint" ? Number(value) : value;
}
