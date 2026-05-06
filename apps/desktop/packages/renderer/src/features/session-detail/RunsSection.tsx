import { useState } from "react";

import type { RunSummary, SessionId } from "@taugentic/desktop-shared";

import { Button } from "@/components/ui/button";
import { Collapsible } from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { StatusDot } from "@/components/ui/status-dot";
import { RecipePicker } from "@/features/recipe-picker/RecipePicker";

import { runStatusTone } from "@/features/run-status/presentation";
import { useSessionRunsQuery } from "@/lib/queries/session-queries";
import { useStartRunMutation } from "@/lib/queries/session-mutations";

import { describeRunStatus, splitLatestAndOlderRuns } from "./formatters";
import { SectionFeedback } from "./section-feedback";
import { SectionHeader } from "./section-header";

export interface RunsSectionProps {
  sessionId: SessionId;
  onRunStarted?: () => void;
  composerPlacement?: "top" | "bottom";
}

export function RunsSection({
  composerPlacement = "top",
  onRunStarted,
  sessionId,
}: RunsSectionProps) {
  const query = useSessionRunsQuery(sessionId);
  const runs = query.data ?? [];
  const hasLoaded = query.data !== undefined;
  const errorMessage = query.error ? toErrorMessage(query.error) : null;
  const { latest, older } = splitLatestAndOlderRuns(runs);
  const composer = <RunComposer onStarted={() => onRunStarted?.()} sessionId={sessionId} />;

  return (
    <section className="flex flex-col gap-2 px-3 py-3" data-section="runs">
      <SectionHeader
        count={runs.length}
        errorMessage={errorMessage}
        hasLoaded={hasLoaded}
        label="runs"
        pending={query.isFetching}
      />
      {composerPlacement === "top" ? composer : null}
      <SectionFeedback
        errorMessage={errorMessage}
        hasLoaded={hasLoaded}
        isEmpty={latest === null}
        itemsLabel="runs"
      />
      {latest !== null ? (
        <>
          <RunRow run={latest} />
          {older.length > 0 ? (
            <Collapsible.Root>
              <Collapsible.Trigger
                className="px-0 text-left font-[var(--font-mono)] text-[11px] uppercase tracking-[0.18em] text-[var(--fg-dim)] hover:text-[var(--fg)]"
                type="button"
              >
                Show {older.length} older run{older.length === 1 ? "" : "s"}
              </Collapsible.Trigger>
              <Collapsible.Content>
                <div className="flex flex-col gap-1 pt-1">
                  {older.map((run) => (
                    <RunRow key={run.id} run={run} />
                  ))}
                </div>
              </Collapsible.Content>
            </Collapsible.Root>
          ) : null}
        </>
      ) : null}
      {composerPlacement === "bottom" ? composer : null}
    </section>
  );
}

function RunRow({ run }: { run: RunSummary }) {
  return (
    <div
      className="flex items-start gap-2 font-[var(--font-mono)] text-[12px] text-[var(--fg)]"
      data-run-id={run.id}
    >
      <StatusDot className="mt-[6px] shrink-0" tone={runStatusTone(run.status)} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-[var(--fg)]">{run.objective}</span>
          <span className="ml-auto shrink-0 text-[10px] uppercase tracking-[0.18em] text-[var(--fg-dim)]">
            {describeRunStatus(run.status)}
          </span>
        </div>
        <div className="truncate text-[11px] text-[var(--fg-mute)]">{run.id}</div>
      </div>
    </div>
  );
}

function RunComposer({ onStarted, sessionId }: { onStarted: () => void; sessionId: SessionId }) {
  const [objective, setObjective] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const startRun = useStartRunMutation(sessionId);

  async function submit() {
    const trimmed = objective.trim();
    if (trimmed.length === 0 || startRun.isPending) {
      return;
    }
    setErrorMessage(null);
    try {
      await startRun.mutateAsync({ objective: trimmed });
      setObjective("");
      onStarted();
    } catch (error) {
      setErrorMessage(toErrorMessage(error));
    }
  }

  return (
    <div className="flex flex-col gap-1 border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-2">
      <div className="flex items-center gap-1">
        <Input
          aria-label="Run objective"
          className="h-7 flex-1 border-[var(--border)] bg-[var(--bg)] px-2 font-[var(--font-mono)] text-[12px] text-[var(--fg)] placeholder:text-[var(--fg-mute)]"
          disabled={startRun.isPending}
          onChange={(event) => setObjective(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void submit();
            }
          }}
          placeholder="start run with objective…"
          type="text"
          value={objective}
        />
        <RecipePicker onRunStarted={onStarted} sessionId={sessionId} />
        <Button
          className="h-7 px-2 text-[10px] uppercase tracking-[0.18em]"
          disabled={startRun.isPending || objective.trim().length === 0}
          onClick={() => void submit()}
          size="sm"
          type="button"
          variant="secondary"
        >
          {startRun.isPending ? "…" : "start"}
        </Button>
      </div>
      {errorMessage !== null ? (
        <div className="font-[var(--font-mono)] text-[11px] text-[var(--status-failed)]">
          error: {errorMessage}
        </div>
      ) : null}
    </div>
  );
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
