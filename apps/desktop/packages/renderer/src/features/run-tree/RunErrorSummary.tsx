import type { JSX } from "react";

import type { RunEventDelta, RunListEntry, ValidationError } from "@taugentic/desktop-shared";

export function RunErrorSummary({
  run,
  timelineEvents,
  validationError,
}: {
  run: RunListEntry;
  timelineEvents: RunEventDelta[];
  validationError: ValidationError | null;
}): JSX.Element | null {
  const terminalError = latestTerminalError(timelineEvents);
  if (validationError === null && run.status !== "failed" && terminalError === null) {
    return null;
  }

  return (
    <section
      className="mb-3 flex flex-col gap-2 rounded-[var(--radius-sm)] border border-rose-400/40 bg-rose-500/10 px-2 py-2 font-[var(--font-mono)] text-[11px] text-rose-100"
      data-run-error-summary=""
    >
      <div className="text-[10px] uppercase tracking-[0.16em] text-rose-200">Run Error</div>
      {validationError !== null ? (
        <ErrorField label="contract" value={validationError.kind} />
      ) : null}
      {terminalError !== null ? <ErrorField label="event" value={terminalError.detail} /> : null}
      {terminalError?.backtrace ? (
        <pre className="whitespace-pre-wrap">{terminalError.backtrace}</pre>
      ) : null}
      {validationError !== null ? (
        <pre className="whitespace-pre-wrap">{JSON.stringify(validationError.value, null, 2)}</pre>
      ) : null}
    </section>
  );
}

function latestTerminalError(
  events: RunEventDelta[],
): { backtrace: string | null; detail: string } | null {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index]?.event;
    if (!event || !("run" in event)) {
      continue;
    }
    if (event.run.status !== "failed" && event.run.status !== "budgetExceeded") {
      continue;
    }
    return {
      backtrace: extractBacktrace(event.run.detail),
      detail: event.run.detail,
    };
  }
  return null;
}

function extractBacktrace(detail: string): string | null {
  const marker = "backtrace:";
  const index = detail.toLowerCase().indexOf(marker);
  if (index === -1) {
    return null;
  }
  return detail.slice(index + marker.length).trim();
}

function ErrorField({ label, value }: { label: string; value: string }): JSX.Element {
  return (
    <div className="grid grid-cols-[5rem_minmax(0,1fr)] gap-2">
      <span className="uppercase tracking-[0.14em] text-rose-200">{label}</span>
      <span className="min-w-0 break-words">{value}</span>
    </div>
  );
}
