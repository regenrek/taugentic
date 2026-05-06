import { useState, type JSX } from "react";

import type { RunListEntry, SessionId } from "@taugentic/desktop-shared";

import { Button } from "@/components/ui/button";
import { useForkRunMutation } from "@/lib/queries/session-mutations";

const TERMINAL_STATUSES = new Set(["completed", "failed", "budgetExceeded", "cancelled"]);

export function RunReplayControl({
  run,
  sessionId,
}: {
  run: RunListEntry;
  sessionId: SessionId | null;
}): JSX.Element | null {
  const parentEventSeq = run.lastEventSeq ?? null;
  const canReplay =
    sessionId !== null && parentEventSeq !== null && TERMINAL_STATUSES.has(run.status);

  if (!canReplay) {
    return null;
  }

  return <RunReplayControlBound parentEventSeq={parentEventSeq} run={run} sessionId={sessionId} />;
}

function RunReplayControlBound({
  parentEventSeq,
  run,
  sessionId,
}: {
  parentEventSeq: bigint;
  run: RunListEntry;
  sessionId: SessionId;
}): JSX.Element {
  const [isOpen, setIsOpen] = useState(false);
  const [objective, setObjective] = useState(run.objectivePreview ?? "");
  const forkRun = useForkRunMutation(sessionId);

  async function replay() {
    await forkRun?.mutateAsync({
      objective: objective.trim().length === 0 ? null : objective,
      parentEventSeq,
      parentRunId: run.id,
    });
    setIsOpen(false);
  }

  return (
    <div className="relative">
      <Button
        className="h-6 px-1.5 text-[9px] tracking-[0.14em]"
        onClick={() => setIsOpen(true)}
        size="sm"
        type="button"
        variant="secondary"
      >
        Replay
      </Button>
      {isOpen ? (
        <div
          aria-label="Replay run"
          className="absolute right-0 z-20 mt-1 w-72 rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-raised)] p-2 shadow-lg"
          role="dialog"
        >
          <label className="flex flex-col gap-1 text-[11px] text-[var(--fg-dim)]">
            <span className="uppercase tracking-[0.16em]">Replay objective</span>
            <textarea
              className="min-h-20 resize-y border border-[var(--border)] bg-[var(--bg-sunken)] px-2 py-1.5 text-[12px] text-[var(--fg)]"
              onChange={(event) => setObjective(event.currentTarget.value)}
              value={objective}
            />
          </label>
          {forkRun.error ? (
            <div className="mt-2 text-[11px] text-[var(--status-failed)]">
              {toErrorMessage(forkRun.error)}
            </div>
          ) : null}
          <div className="mt-2 flex justify-end gap-1">
            <Button onClick={() => setIsOpen(false)} size="sm" type="button" variant="ghost">
              Cancel
            </Button>
            <Button disabled={forkRun.isPending} onClick={replay} size="sm" type="button">
              {forkRun.isPending ? "Replaying" : "Replay"}
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
