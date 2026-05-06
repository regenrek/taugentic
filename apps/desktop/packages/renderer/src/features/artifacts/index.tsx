import { useQueryClient } from "@tanstack/react-query";
import { useActorRef, useSelector } from "@xstate/react";

import type { SessionId } from "@taugentic/desktop-shared";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { listArtifacts } from "@/lib/ipc/api";
import { subscribeArtifactStream } from "@/lib/ipc/stream";
import { queryKeys } from "@/lib/queries/keys";
import { useSessionArtifactsQuery } from "@/lib/queries/session-queries";

import { sessionArtifactMachine } from "./connection";

export function ArtifactsPanel({
  onSessionInvalid,
  sessionId,
}: {
  onSessionInvalid: (sessionId: SessionId) => void;
  sessionId: SessionId;
}) {
  return (
    <Card className="glass-panel rounded-3xl border-white/10">
      <CardHeader className="pb-4">
        <CardTitle className="text-white">Artifacts</CardTitle>
        <CardDescription>
          Browse captured outputs for the active session and keep the selected artifact pinned in
          context.
        </CardDescription>
      </CardHeader>
      <SessionArtifactsPanel
        key={sessionId}
        onSessionInvalid={onSessionInvalid}
        sessionId={sessionId}
      />
    </Card>
  );
}

function SessionArtifactsPanel({
  onSessionInvalid,
  sessionId,
}: {
  onSessionInvalid: (sessionId: SessionId) => void;
  sessionId: SessionId;
}) {
  const qc = useQueryClient();
  const artifactsQuery = useSessionArtifactsQuery(sessionId);
  const actorRef = useActorRef(sessionArtifactMachine, {
    input: {
      deps: {
        hydrateSnapshot() {},
        listArtifacts: (targetSessionId) =>
          qc.fetchQuery({
            queryKey: queryKeys.sessionArtifacts(targetSessionId),
            queryFn: () => listArtifacts(targetSessionId, {}),
          }),
        async subscribeArtifactStream(targetSessionId, afterCursor, onMessage, onError) {
          return subscribeArtifactStream(targetSessionId, afterCursor, onMessage, onError);
        },
      },
      onMissingSession: onSessionInvalid,
      sessionId,
    },
  });
  const artifacts = artifactsQuery.data ?? [];
  const currentArtifactId = useSelector(actorRef, (snapshot) => snapshot.context.currentArtifactId);
  const errorMessage = useSelector(actorRef, (snapshot) => snapshot.context.errorMessage);
  const isHydrating = useSelector(actorRef, (snapshot) => snapshot.context.isHydrating);
  const currentSessionId = useSelector(actorRef, (snapshot) => snapshot.context.sessionId);
  const selectedArtifact =
    currentArtifactId === null
      ? null
      : (artifacts.find((artifact) => artifact.id === currentArtifactId) ?? null);

  return (
    <CardContent className="grid gap-5">
      <div className="grid gap-3 sm:grid-cols-2">
        <MetricCard label="Session" mono value={currentSessionId} />
        <MetricCard label="Artifacts" value={String(artifacts.length)} />
      </div>

      {errorMessage ? (
        <div className="rounded-2xl border border-rose-300/15 bg-rose-500/10 px-4 py-3 text-sm text-rose-100">
          error: {errorMessage}
        </div>
      ) : null}

      {artifacts.length === 0 ? (
        <p className="rounded-2xl border border-dashed border-white/12 bg-white/3 px-4 py-6 text-sm text-slate-300">
          {isHydrating
            ? "Loading artifacts for the selected session..."
            : "No artifacts yet for the selected session."}
        </p>
      ) : (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
          <div className="grid gap-3">
            <div className="text-[11px] uppercase tracking-[0.24em] text-slate-400">
              artifact list
            </div>
            {artifacts.map((artifact) => {
              const selected = artifact.id === currentArtifactId;
              return (
                <button
                  key={artifact.id}
                  className={[
                    "rounded-3xl border px-5 py-4 text-left transition",
                    selected
                      ? "border-amber-300/30 bg-amber-300/10"
                      : "border-white/8 bg-white/4 hover:border-white/14 hover:bg-white/6",
                  ].join(" ")}
                  onClick={() =>
                    actorRef.send({ type: "artifactSelected", artifactId: artifact.id })
                  }
                  type="button"
                >
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                    <div className="space-y-2">
                      <strong className="block text-white">{artifact.kind}</strong>
                      <div className="font-mono text-[11px] text-slate-400">{artifact.id}</div>
                      <div className="font-mono text-[11px] text-slate-400">
                        run: {artifact.runId}
                      </div>
                    </div>
                    <Badge variant={selected ? "accent" : "outline"}>{artifact.kind}</Badge>
                  </div>
                  <div className="mt-3 break-all text-sm leading-6 text-slate-300">
                    {artifact.storagePath}
                  </div>
                </button>
              );
            })}
          </div>

          <div className="grid gap-3">
            <div className="text-[11px] uppercase tracking-[0.24em] text-slate-400">
              selected artifact
            </div>
            {selectedArtifact ? (
              <article className="rounded-3xl border border-white/8 bg-white/4 px-5 py-4">
                <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                  <div className="space-y-2">
                    <strong className="block text-white">{selectedArtifact.kind}</strong>
                    <div className="font-mono text-[11px] text-slate-400">
                      {selectedArtifact.id}
                    </div>
                    <div className="font-mono text-[11px] text-slate-400">
                      run: {selectedArtifact.runId}
                    </div>
                  </div>
                  <Badge variant="accent">{selectedArtifact.kind}</Badge>
                </div>
                <div className="mt-3 break-all text-sm leading-6 text-slate-300">
                  {selectedArtifact.storagePath}
                </div>
              </article>
            ) : (
              <p className="rounded-2xl border border-dashed border-white/12 bg-white/3 px-4 py-6 text-sm text-slate-300">
                Select an artifact to inspect its details.
              </p>
            )}
          </div>
        </div>
      )}
    </CardContent>
  );
}

function MetricCard({
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
