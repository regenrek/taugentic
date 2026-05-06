/*
 * RawTab.
 *
 * Read-only monospace dump of the focused session's most recent activity
 * page items as JSON. Reads from the existing `useSessionActivityQuery`
 * hook; the JSON serializer handles `bigint` cursor sequences via a
 * replacer (raw view shows them as decimal strings to keep operator-
 * facing copy/paste identical to backend logs).
 */

import type { PublicActivityPageItem, SessionId } from "@taugentic/desktop-shared";

import { useSessionActivityQuery } from "@/lib/queries/session-queries";

import { pickRecentActivity } from "../selectors/focused-run";

const MAX_RAW_ENTRIES = 200;

export interface RawTabProps {
  sessionId: SessionId;
}

export function RawTab({ sessionId }: RawTabProps) {
  const query = useSessionActivityQuery(sessionId);
  const items = pickRecentActivity(query.data, MAX_RAW_ENTRIES);
  const hasLoaded = query.data !== undefined;
  const errorMessage = query.error
    ? query.error instanceof Error
      ? query.error.message
      : toUnknownErrorMessage(query.error)
    : null;

  return (
    <section
      aria-label="Raw events"
      className="flex flex-col gap-2 px-3 py-3"
      data-agent-visualization-tab="raw"
      data-raw-count={items.length}
    >
      <header className="flex items-center justify-between text-[10px] uppercase tracking-[0.18em] text-[var(--fg-dim)]">
        <span>raw events</span>
        <span className="text-[var(--fg-mute)]">
          {items.length} {items.length === 1 ? "entry" : "entries"}
        </span>
      </header>
      {errorMessage !== null ? (
        <div className="font-[var(--font-mono)] text-[11px] text-[var(--status-failed)]">
          error: {errorMessage}
        </div>
      ) : null}
      {hasLoaded && items.length === 0 ? (
        <div className="font-[var(--font-mono)] text-[11px] text-[var(--fg-mute)]">
          no raw events
        </div>
      ) : null}
      {items.length > 0 ? (
        <pre
          className="max-h-[420px] overflow-auto border border-[var(--border)] bg-[var(--bg-sunken)] px-2 py-2 font-[var(--font-mono)] text-[11px] leading-5 text-[var(--fg)]"
          data-raw-events
        >
          {serializeItems(items)}
        </pre>
      ) : null}
    </section>
  );
}

function toUnknownErrorMessage(error: unknown): string {
  if (
    typeof error === "string" ||
    typeof error === "number" ||
    typeof error === "boolean" ||
    typeof error === "bigint"
  ) {
    return String(error);
  }
  return JSON.stringify(error) ?? "unknown error";
}

function serializeItems(items: readonly PublicActivityPageItem[]): string {
  return items.map((item) => JSON.stringify(item, bigintReplacer, 2)).join("\n");
}

function bigintReplacer(_key: string, value: unknown): unknown {
  if (typeof value === "bigint") {
    return value.toString();
  }
  return value;
}
