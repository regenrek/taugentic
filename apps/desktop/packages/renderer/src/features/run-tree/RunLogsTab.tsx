import type { JSX } from "react";

import type { RunEventDelta } from "@taugentic/desktop-shared";

import { EmptyDetailState } from "./RunTimelineTab";

type AgentStreamEvent = Extract<RunEventDelta["event"], { agentStream: unknown }>;
type AgentStreamFrame = AgentStreamEvent["agentStream"]["frame"];

export function RunLogsTab({
  events,
  isFetching,
}: {
  events: RunEventDelta[];
  isFetching: boolean;
}): JSX.Element {
  if (events.length === 0) {
    return (
      <EmptyDetailState message={isFetching ? "loading capsule logs" : "no capsule logs found"} />
    );
  }

  return (
    <ol className="flex flex-col gap-2 font-[var(--font-mono)] text-[11px]">
      {events.map((delta) => (
        <li
          className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-sunken)] px-2 py-2"
          key={delta.seq.toString()}
        >
          <div className="mb-1 text-[10px] uppercase tracking-[0.14em] text-[var(--fg-dim)]">
            #{delta.seq.toString()} · {eventKind(delta)}
          </div>
          <pre className="whitespace-pre-wrap break-words text-[var(--fg)]">
            {formatLogMessage(delta)}
          </pre>
        </li>
      ))}
    </ol>
  );
}

function eventKind(delta: RunEventDelta): string {
  const event = delta.event;
  if ("run" in event) return `run ${event.run.status}`;
  if ("agentStream" in event) return `agent ${event.agentStream.frame.kind}`;
  if ("approval" in event) return `approval ${event.approval.phase}`;
  if ("artifact" in event) return `artifact ${event.artifact.artifact.kind}`;
  if ("contextReceipt" in event) return `receipt ${event.contextReceipt.phase}`;
  if ("conflict" in event) return `conflict ${event.conflict.phase}`;
  if ("budget" in event) return `budget ${event.budget.phase}`;
  return "event";
}

function formatLogMessage(delta: RunEventDelta): string {
  const event = delta.event;
  if ("run" in event) return event.run.detail;
  if ("agentStream" in event) return formatAgentFrame(event.agentStream.frame);
  if ("approval" in event) return JSON.stringify(event.approval, bigintReplacer, 2);
  if ("artifact" in event) return JSON.stringify(event.artifact, bigintReplacer, 2);
  if ("contextReceipt" in event) return JSON.stringify(event.contextReceipt, bigintReplacer, 2);
  if ("conflict" in event) return JSON.stringify(event.conflict, bigintReplacer, 2);
  if ("budget" in event) return JSON.stringify(event.budget, bigintReplacer, 2);
  return JSON.stringify(event, bigintReplacer, 2);
}

function formatAgentFrame(frame: AgentStreamFrame): string {
  switch (frame.kind) {
    case "toolCallStarted":
      return `tool started: ${frame.toolName}\n${frame.input}`;
    case "toolCallCompleted":
      return `tool ${frame.outcome}`;
    case "tokenUsageUpdated":
      return `token usage total=${frame.totalTokens?.toString() ?? "unknown"} context=${frame.modelContextWindow?.toString() ?? "unknown"}`;
    case "pendingStateChanged":
      return `pending ${frame.state}`;
    case "assistantMessageDelta":
      return frame.delta;
    case "toolCallProgressed":
      return frame.delta;
    default:
      return frame.kind;
  }
}

function bigintReplacer(_key: string, value: unknown): unknown {
  return typeof value === "bigint" ? value.toString() : value;
}
