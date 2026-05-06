import { Clock3, Plus, RefreshCcw } from "lucide-react";

import type { SessionId } from "@taugentic/desktop-shared";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";

import { useSessionsPanelModel } from "./model";

export function SessionsPanel({
  currentSessionId,
  onSessionChange,
}: {
  currentSessionId: SessionId | null;
  onSessionChange: (sessionId: SessionId | null) => void;
}) {
  const { openSession, refreshSessions, selectSession, setDraftTitle, state } =
    useSessionsPanelModel(currentSessionId, onSessionChange);

  const isOpening = state.pendingAction === "open";
  const isRefreshing = state.pendingAction === "refresh";
  const { draftTitle, errorMessage, sessions } = state;

  return (
    <Card className="glass-panel rounded-3xl border-white/10">
      <CardHeader className="pb-5">
        <div className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
          <div className="space-y-2">
            <CardTitle className="text-white">Sessions</CardTitle>
            <CardDescription>
              Open a coding workspace, keep the active session pinned, and reuse it across runs and
              review surfaces.
            </CardDescription>
          </div>
          <Button
            disabled={isOpening || isRefreshing}
            onClick={() => void refreshSessions()}
            type="button"
            variant="secondary"
          >
            <RefreshCcw className="size-3.5" />
            {isRefreshing ? "Refreshing..." : "Refresh"}
          </Button>
        </div>
      </CardHeader>

      <CardContent className="space-y-6">
        <div className="grid gap-3 xl:grid-cols-[minmax(0,1fr)_auto]">
          <Input
            id="session-title"
            onChange={(event) => setDraftTitle(event.currentTarget.value)}
            placeholder="Build daemon app server"
            type="text"
            value={draftTitle}
          />
          <Button
            disabled={isOpening || draftTitle.trim().length === 0}
            onClick={() => void openSession()}
            type="button"
          >
            <Plus className="size-4" />
            {isOpening ? "Opening..." : "Open session"}
          </Button>
        </div>

        <div className="grid gap-3">
          {sessions.length === 0 ? (
            <div className="rounded-2xl border border-dashed border-white/12 bg-white/3 px-5 py-8 text-sm text-slate-300">
              No sessions yet. Open your first session here.
            </div>
          ) : (
            sessions.map((session) => {
              const selected = session.id === currentSessionId;
              return (
                <button
                  key={session.id}
                  className={[
                    "group rounded-3xl border px-5 py-4 text-left transition",
                    selected
                      ? "border-amber-300/30 bg-amber-300/10 shadow-[0_18px_40px_rgba(255,147,34,0.12)]"
                      : "border-white/8 bg-white/4 hover:border-white/14 hover:bg-white/6",
                  ].join(" ")}
                  onClick={() => selectSession(session.id)}
                  type="button"
                >
                  <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
                    <div className="space-y-2">
                      <div className="text-base font-medium text-white">{session.title}</div>
                      <div className="flex flex-wrap gap-2">
                        <Badge variant={selected ? "accent" : "outline"}>{session.status}</Badge>
                        {selected ? <Badge variant="secondary">current</Badge> : null}
                      </div>
                      <div className="font-mono text-[11px] text-slate-400">{session.id}</div>
                    </div>
                    <div className="flex items-center gap-2 text-xs text-slate-400">
                      <Clock3 className="size-3.5" />
                      ready for run orchestration
                    </div>
                  </div>
                </button>
              );
            })
          )}
        </div>

        {errorMessage ? (
          <div className="rounded-2xl border border-rose-300/15 bg-rose-500/10 px-4 py-3 text-sm text-rose-100">
            error: {errorMessage}
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}
