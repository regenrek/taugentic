import { Activity, Rocket, Sparkles } from "lucide-react";

import type { SessionId } from "@taugentic/desktop-shared";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  getRunStatusPresentation,
  type RunPresentationStatus,
} from "@/features/run-status/presentation";

import { useSessionRunsModel } from "./model";
import { RunFlowVisual } from "./run-flow-visual";

export function RunsPanel({
  onSessionInvalid,
  sessionId,
  onRunStarted,
}: {
  onSessionInvalid: (sessionId: SessionId) => void;
  sessionId: SessionId;
  onRunStarted?: () => void;
}) {
  return (
    <Card className="glass-panel rounded-3xl border-white/10">
      <CardHeader className="pb-4">
        <CardTitle className="text-white">Runs</CardTitle>
        <CardDescription>
          Launch new agent work, watch stream hydration, and inspect durable run activity as it
          lands from the daemon.
        </CardDescription>
      </CardHeader>
      <SessionRunsPanel
        key={sessionId}
        onRunStarted={onRunStarted}
        onSessionInvalid={onSessionInvalid}
        sessionId={sessionId}
      />
    </Card>
  );
}

function SessionRunsPanel({
  onSessionInvalid,
  sessionId,
  onRunStarted,
}: {
  onSessionInvalid: (sessionId: SessionId) => void;
  sessionId: SessionId;
  onRunStarted?: () => void;
}) {
  const runModel = useSessionRunsModel(sessionId, {
    onRunStarted,
    onSessionInvalid,
  });
  const { commandErrorMessage, draftObjective, isStarting } = runModel;
  const streamLabel = formatStreamStatus(runModel.streamStatus, runModel.isHydrating);
  const latestRunStatus = runModel.runs[0]?.status ?? null;

  return (
    <CardContent className="grid gap-6">
      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(320px,0.9fr)]">
        <div className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-3">
            <SummaryChip label="Active session" mono value={sessionId} />
            <SummaryChip label="Stream" value={streamLabel} />
            <SummaryChip label="Latest run" value={latestRunStatus ?? "none yet"} />
          </div>

          <div className="grid gap-3 xl:grid-cols-[minmax(0,1fr)_auto]">
            <Input
              id={`run-objective-${sessionId}`}
              onChange={(event) => runModel.setDraftObjective(event.currentTarget.value)}
              placeholder="Ship app server hard cut"
              type="text"
              value={draftObjective}
            />
            <Button
              disabled={isStarting || draftObjective.trim().length === 0}
              onClick={() => runModel.startRun()}
              type="button"
            >
              <Rocket className="size-4" />
              {isStarting ? "Starting..." : "Start run"}
            </Button>
          </div>

          {runModel.errorMessage ? <ErrorBanner message={runModel.errorMessage} /> : null}
          {commandErrorMessage ? <ErrorBanner message={commandErrorMessage} /> : null}
        </div>
        <RunFlowVisual
          draftObjective={draftObjective}
          isHydrating={runModel.isHydrating}
          recentEvents={runModel.recentEvents}
          sessionId={sessionId}
          streamStatus={runModel.streamStatus}
        />
      </div>

      <div className="grid gap-6 xl:grid-cols-2">
        <section className="space-y-3">
          <div className="flex items-center gap-2 text-sm font-medium text-white">
            <Sparkles className="size-4 text-amber-200" />
            Durable runs
          </div>
          {runModel.runs.length === 0 ? (
            <p className="rounded-2xl border border-dashed border-white/12 bg-white/3 px-4 py-6 text-sm text-slate-300">
              {runModel.isHydrating
                ? "Loading runs for the selected session..."
                : "No runs yet for the selected session."}
            </p>
          ) : (
            <div className="grid gap-3">
              {runModel.runs.map((run) => (
                <article
                  key={run.id}
                  className="rounded-3xl border border-white/8 bg-white/4 px-5 py-4 transition hover:border-white/14 hover:bg-white/6"
                >
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                    <div className="space-y-2">
                      <strong className="block text-white">{run.objective}</strong>
                      <div className="font-mono text-[11px] text-slate-400">{run.id}</div>
                    </div>
                    <RunStatusBadge status={run.status} />
                  </div>
                </article>
              ))}
            </div>
          )}
        </section>

        <section className="space-y-3">
          <div className="flex items-center gap-2 text-sm font-medium text-white">
            <Activity className="size-4 text-amber-200" />
            Recent run events
          </div>
          {runModel.recentEvents.length === 0 ? (
            <p className="rounded-2xl border border-dashed border-white/12 bg-white/3 px-4 py-6 text-sm text-slate-300">
              {runModel.isHydrating
                ? "Waiting for run activity..."
                : "No run events received for this session yet."}
            </p>
          ) : (
            <div className="grid gap-3">
              {runModel.recentEvents.map((event) => (
                <article
                  key={event.cursor.sequence.toString()}
                  className="rounded-3xl border border-white/8 bg-white/4 px-5 py-4"
                >
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                    <div className="space-y-2">
                      <div className="font-mono text-[11px] text-slate-400">
                        {event.event.run.runId}
                      </div>
                      <div className="text-sm leading-6 text-slate-200">
                        {event.event.run.detail}
                      </div>
                    </div>
                    <RunStatusBadge status={event.event.run.status} />
                  </div>
                </article>
              ))}
            </div>
          )}
        </section>
      </div>
    </CardContent>
  );
}

export function RunStatusBadge({ status }: { status: RunPresentationStatus }) {
  const presentation = getRunStatusPresentation(status);

  return <Badge variant={presentation.badgeVariant}>{presentation.label}</Badge>;
}

function formatStreamStatus(
  streamStatus: "connecting" | "live" | "error",
  isHydrating: boolean,
): string {
  if (streamStatus === "error") {
    return "error";
  }
  if (isHydrating) {
    return "hydrating";
  }
  return streamStatus === "live" ? "live" : "connecting";
}

function ErrorBanner({ message }: { message: string }) {
  return (
    <div className="rounded-2xl border border-rose-300/15 bg-rose-500/10 px-4 py-3 text-sm text-rose-100">
      error: {message}
    </div>
  );
}

function SummaryChip({
  label,
  mono = false,
  value,
}: {
  label: string;
  mono?: boolean;
  value: string;
}) {
  return (
    <div className="rounded-2xl border border-white/8 bg-white/5 px-4 py-3">
      <div className="text-[11px] uppercase tracking-[0.24em] text-slate-400">{label}</div>
      <div className={["mt-2 text-sm text-white", mono ? "font-mono" : "font-medium"].join(" ")}>
        {value}
      </div>
    </div>
  );
}
