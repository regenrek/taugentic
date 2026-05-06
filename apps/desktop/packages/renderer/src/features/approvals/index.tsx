import { useQueryClient } from "@tanstack/react-query";
import { useActorRef, useSelector } from "@xstate/react";

import type { SessionId } from "@taugentic/desktop-shared";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { decideApproval, listApprovals } from "@/lib/ipc/api";
import { subscribeApprovalStream } from "@/lib/ipc/stream";
import { queryKeys } from "@/lib/queries/keys";
import { useSessionApprovalsQuery } from "@/lib/queries/session-queries";

import { sessionApprovalMachine } from "./connection";
import { selectSessionApprovalViewState } from "./model";

export function ApprovalsPanel({
  onSessionInvalid,
  sessionId,
}: {
  onSessionInvalid: (sessionId: SessionId) => void;
  sessionId: SessionId;
}) {
  return (
    <Card className="glass-panel rounded-3xl border-white/10">
      <CardHeader className="pb-4">
        <CardTitle className="text-white">Approvals</CardTitle>
        <CardDescription>
          Review daemon-owned approval requests without losing session context.
        </CardDescription>
      </CardHeader>
      <SessionApprovalStateView
        key={sessionId}
        onSessionInvalid={onSessionInvalid}
        sessionId={sessionId}
      />
    </Card>
  );
}

function SessionApprovalStateView({
  onSessionInvalid,
  sessionId,
}: {
  onSessionInvalid: (sessionId: SessionId) => void;
  sessionId: SessionId;
}) {
  const qc = useQueryClient();
  const approvalsQuery = useSessionApprovalsQuery(sessionId);
  const actorRef = useActorRef(sessionApprovalMachine, {
    input: {
      deps: {
        async decideApproval(targetSessionId, approvalId, decision) {
          await decideApproval(targetSessionId, approvalId, decision);
        },
        hydrateSnapshot() {},
        listApprovals: (targetSessionId) =>
          qc.fetchQuery({
            queryKey: queryKeys.sessionApprovals(targetSessionId),
            queryFn: () => listApprovals(targetSessionId, {}),
          }),
        async subscribeApprovalStream(targetSessionId, afterCursor, onMessage, onError) {
          return subscribeApprovalStream(targetSessionId, afterCursor, onMessage, onError);
        },
      },
      onMissingSession: onSessionInvalid,
      sessionId,
    },
  });
  const state = useSelector(actorRef, selectSessionApprovalViewState);
  const approvals = approvalsQuery.data ?? [];
  const { commandErrorMessage, pendingApprovalId, pendingDecision } = state;

  return (
    <CardContent className="grid gap-5">
      <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <MetricCard label="Session" mono value={state.sessionId} />
        <MetricCard label="Stream" value={state.streamStatus} />
        <MetricCard label="Requests" value={String(approvals.length)} />
        <MetricCard
          label="Last sequence"
          mono
          value={state.lastSequence === null ? "none" : state.lastSequence.toString()}
        />
      </div>

      {state.errorMessage || commandErrorMessage ? (
        <div className="rounded-2xl border border-rose-300/15 bg-rose-500/10 px-4 py-3 text-sm text-rose-100">
          error: {commandErrorMessage ?? state.errorMessage}
        </div>
      ) : null}

      {approvals.length === 0 ? (
        <p className="rounded-2xl border border-dashed border-white/12 bg-white/3 px-4 py-6 text-sm text-slate-300">
          {state.streamStatus === "connecting"
            ? "Loading approvals for this session..."
            : "No approvals yet for this session."}
        </p>
      ) : (
        <div className="grid gap-3">
          {approvals.map((approval) => (
            <article
              key={approval.id}
              className="rounded-3xl border border-white/8 bg-white/4 px-5 py-4"
            >
              <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                <div className="space-y-2">
                  <strong className="block text-white">{approval.scope}</strong>
                  <div className="font-mono text-[11px] text-slate-400">{approval.id}</div>
                  <div className="font-mono text-[11px] text-slate-400">run: {approval.runId}</div>
                </div>
                <Badge variant="outline">{approval.scope}</Badge>
              </div>
              <p className="mt-3 text-sm leading-6 text-slate-200">{approval.reason}</p>
              <div className="mt-4 flex flex-wrap gap-2">
                <Button
                  disabled={pendingApprovalId !== null}
                  onClick={() =>
                    actorRef.send({
                      type: "approvalDecisionRequested",
                      approvalId: approval.id,
                      decision: "approved",
                    })
                  }
                  size="sm"
                  type="button"
                >
                  {pendingApprovalId === approval.id && pendingDecision === "approved"
                    ? "Approving..."
                    : "Approve"}
                </Button>
                <Button
                  disabled={pendingApprovalId !== null}
                  onClick={() =>
                    actorRef.send({
                      type: "approvalDecisionRequested",
                      approvalId: approval.id,
                      decision: "rejected",
                    })
                  }
                  size="sm"
                  type="button"
                  variant="outline"
                >
                  {pendingApprovalId === approval.id && pendingDecision === "rejected"
                    ? "Rejecting..."
                    : "Reject"}
                </Button>
              </div>
            </article>
          ))}
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
