import type { JSX } from "react";

import type { RunTimeline } from "@taugentic/desktop-shared";

import {
  createTimelineLanes,
  formatTimelineEvent,
  formatTimelineRange,
  shortRunId,
} from "./timeline-model";

export function RunTimelineTab({
  isFetching,
  timeline,
}: {
  isFetching: boolean;
  timeline: RunTimeline | null;
}): JSX.Element {
  return <RunTimelineTabView isFetching={isFetching} timeline={timeline} />;
}

function RunTimelineTabView({
  isFetching,
  timeline,
}: {
  isFetching: boolean;
  timeline: RunTimeline | null;
}): JSX.Element {
  if (timeline === null) {
    return <EmptyDetailState message={isFetching ? "loading run timeline" : "no run timeline"} />;
  }
  const lanes = createTimelineLanes(timeline);
  if (lanes.length === 0) {
    return <EmptyDetailState message="no runs in timeline" />;
  }

  return (
    <div className="flex flex-col gap-3 font-[var(--font-mono)] text-[11px]">
      {lanes.map((lane) => (
        <section
          className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-sunken)] px-2 py-2"
          key={lane.run.runId}
        >
          <div className="mb-2 flex flex-wrap items-center gap-2">
            <span className="text-[var(--fg)]">{shortRunId(lane.run.runId)}</span>
            <span className="uppercase tracking-[0.14em] text-[var(--fg-dim)]">
              depth {lane.run.depth}
            </span>
            <span className="uppercase tracking-[0.14em] text-[var(--fg-dim)]">
              {lane.run.status}
            </span>
            <span className="text-[var(--fg-mute)]">{formatTimelineRange(lane.run)}</span>
          </div>
          {lane.events.length > 0 ? (
            <ol className="flex flex-col gap-1 border-l border-[var(--border)]/70 pl-3">
              {lane.events.map((event) => (
                <li className="min-w-0 break-words text-[var(--fg)]" key={event.seq.toString()}>
                  {formatTimelineEvent(event)}
                </li>
              ))}
            </ol>
          ) : (
            <EmptyDetailState message="no timeline events for this run" />
          )}
        </section>
      ))}
    </div>
  );
}

export function EmptyDetailState({ message }: { message: string }): JSX.Element {
  return (
    <div className="font-[var(--font-mono)] text-[11px] text-[var(--fg-mute)]" data-state="empty">
      {message}
    </div>
  );
}
