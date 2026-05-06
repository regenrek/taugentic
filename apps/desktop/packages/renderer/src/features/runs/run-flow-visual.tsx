import type { SessionId } from "@taugentic/desktop-shared";

import type { RunActivityItem } from "./state";

export function RunFlowVisual({
  draftObjective,
  isHydrating,
  recentEvents,
  sessionId,
  streamStatus,
}: {
  draftObjective: string;
  isHydrating: boolean;
  recentEvents: RunActivityItem[];
  sessionId: SessionId;
  streamStatus: "connecting" | "live" | "error";
}) {
  const latestEvent = recentEvents[0];
  const latestStatus = latestEvent?.event.run.status ?? null;

  return (
    <div className="run-flow rounded-[28px] border border-white/10 bg-black/20 p-5">
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="text-[11px] uppercase tracking-[0.26em] text-slate-400">
            Session runner
          </div>
          <h3 className="mt-2 text-lg font-semibold text-white">How this session is flowing</h3>
        </div>
        <div className="font-mono text-[11px] text-slate-400">{sessionId}</div>
      </div>

      <div className="mt-6 grid gap-4 lg:grid-cols-4">
        <FlowNode active detail="workspace selected" label="Session attached" state={sessionId} />
        <FlowNode
          active={draftObjective.trim().length > 0}
          detail="run command draft"
          label="Objective composed"
          state={draftObjective.trim().length > 0 ? draftObjective : "waiting for objective"}
        />
        <FlowNode
          active={streamStatus !== "error"}
          detail="daemon event pipe"
          label="Stream state"
          state={isHydrating ? "hydrating" : streamStatus}
          tone={streamStatus === "error" ? "danger" : "accent"}
        />
        <FlowNode
          active={latestEvent != null}
          detail="latest durable event"
          label="Run activity"
          state={latestStatus ?? "awaiting first event"}
          tone={latestStatus === "completed" ? "success" : "accent"}
        />
      </div>
    </div>
  );
}

function FlowNode({
  active,
  detail,
  label,
  state,
  tone = "default",
}: {
  active: boolean;
  detail: string;
  label: string;
  state: string;
  tone?: "accent" | "danger" | "default" | "success";
}) {
  return (
    <div
      className={[
        "run-flow-node rounded-3xl border px-4 py-4 transition",
        active ? "is-active" : "",
        tone === "accent"
          ? "border-amber-300/18 bg-amber-300/8"
          : tone === "success"
            ? "border-emerald-300/18 bg-emerald-300/8"
            : tone === "danger"
              ? "border-rose-300/18 bg-rose-300/8"
              : "border-white/10 bg-white/4",
      ].join(" ")}
    >
      <div className="text-[11px] uppercase tracking-[0.24em] text-slate-400">{label}</div>
      <div className="mt-3 text-sm font-medium text-white">{state}</div>
      <div className="mt-2 text-xs leading-5 text-slate-400">{detail}</div>
    </div>
  );
}
