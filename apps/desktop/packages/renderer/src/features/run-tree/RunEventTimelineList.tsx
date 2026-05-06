import type { JSX } from "react";

import type { RunEventDelta } from "@taugentic/desktop-shared";

import { EmptyDetailState } from "./RunTimelineTab";

export function RunEventTimelineList({
  events,
  isFetching,
}: {
  events: RunEventDelta[];
  isFetching: boolean;
}): JSX.Element {
  if (events.length === 0) {
    return (
      <EmptyDetailState
        message={isFetching ? "loading capsule timeline" : "no replayed capsule events"}
      />
    );
  }
  return (
    <ol className="flex flex-col gap-1">
      {events.map((event) => (
        <li className="min-w-0 break-words text-[var(--fg)]" key={event.seq.toString()}>
          {formatRunEventDelta(event)}
        </li>
      ))}
    </ol>
  );
}

function formatRunEventDelta(delta: RunEventDelta): string {
  const prefix = `#${delta.seq.toString()}`;
  const event = delta.event;
  if ("run" in event) {
    return `${prefix} run ${event.run.status}: ${event.run.detail}`;
  }
  if ("agentStream" in event) {
    return `${prefix} agent stream ${event.agentStream.frame.kind}`;
  }
  if ("approval" in event) {
    return `${prefix} approval ${event.approval.phase}`;
  }
  if ("artifact" in event) {
    return `${prefix} artifact ${event.artifact.artifact.kind}`;
  }
  if ("contextReceipt" in event) {
    return `${prefix} receipt ${event.contextReceipt.phase}`;
  }
  if ("conflict" in event) {
    return `${prefix} conflict ${event.conflict.phase}`;
  }
  return `${prefix} event`;
}
