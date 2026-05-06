import type { DaemonDiagnostics, WorkflowStatusResult } from "@taugentic/desktop-shared";

import { Badge } from "@/components/ui/badge";
import { useDaemonDiagnosticsQuery } from "@/lib/queries/diagnostics";
import { useWorkflowStatusQuery } from "@/lib/queries/workflow";

export function MissionControlPanel() {
  const diagnostics = useDaemonDiagnosticsQuery();
  const workflow = useWorkflowStatusQuery();
  return (
    <MissionControlPanelView
      errorMessage={diagnostics.error ? toErrorMessage(diagnostics.error) : null}
      isLoading={diagnostics.isLoading}
      snapshot={diagnostics.data}
      workflowStatus={workflow.data}
    />
  );
}

export interface MissionControlPanelViewProps {
  errorMessage: string | null;
  isLoading: boolean;
  snapshot: DaemonDiagnostics | undefined;
  workflowStatus?: WorkflowStatusResult;
}

export function MissionControlPanelView({
  errorMessage,
  isLoading,
  snapshot,
  workflowStatus,
}: MissionControlPanelViewProps) {
  return (
    <section
      aria-label="Mission Control diagnostics"
      className="border-b border-[var(--border)] bg-[var(--bg)] px-3 py-3 font-[var(--font-mono)]"
      data-feature="mission-control"
    >
      <div className="mb-2 flex items-center justify-between gap-2">
        <div className="text-[10px] uppercase tracking-[0.18em] text-[var(--fg-mute)]">
          MISSION CONTROL
        </div>
        {snapshot ? (
          <Badge className="font-mono text-[10px]" variant="outline">
            {formatDuration(snapshot.uptimeMs)}
          </Badge>
        ) : null}
      </div>

      {isLoading && snapshot === undefined ? (
        <div className="text-[12px] text-[var(--fg-dim)]">Loading diagnostics…</div>
      ) : snapshot === undefined ? (
        <div className="text-[12px] text-[var(--status-failed)]">
          error: {errorMessage ?? "diagnostics unavailable"}
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          <div className="grid grid-cols-2 gap-2">
            <Metric label="rpc" value={snapshot.inFlightRpcCount.toString()} />
            <Metric label="runs" value={snapshot.inFlightCapsuleRunCount.toString()} />
            <Metric label="worktrees" value={snapshot.worktreeCount.toString()} />
            <Metric label="claims" value={snapshot.claimCount.toString()} />
          </div>

          <section className="flex flex-col gap-1.5">
            <SectionLabel>Sandbox</SectionLabel>
            <div className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-2 text-[11px] text-[var(--fg)]">
              <div>
                {snapshot.sandbox.os} · {snapshot.sandbox.sandboxKind}
              </div>
              <div className="text-[var(--fg-dim)]">
                network default deny: {formatBoolean(snapshot.sandbox.networkDefaultDeny)}
              </div>
              <div className="text-[var(--fg-dim)]">
                destination allowlist: {formatBoolean(snapshot.sandbox.networkDestinationAllowlist)}
              </div>
            </div>
          </section>

          <section className="flex flex-col gap-1.5">
            <SectionLabel>Workflow Status</SectionLabel>
            <WorkflowStatusPanel status={workflowStatus} />
          </section>

          <section className="flex flex-col gap-1.5">
            <SectionLabel>Token Usage</SectionLabel>
            <div className="grid grid-cols-2 gap-2">
              <Metric
                label="prompt"
                value={formatOptionalBigInt(snapshot.tokenUsage.promptTokens)}
              />
              <Metric
                label="completion"
                value={formatOptionalBigInt(snapshot.tokenUsage.completionTokens)}
              />
              <Metric
                label="cached"
                value={formatOptionalBigInt(snapshot.tokenUsage.cachedTokens)}
              />
              <Metric
                label="reasoning"
                value={formatOptionalBigInt(snapshot.tokenUsage.reasoningTokens)}
              />
            </div>
          </section>

          <section className="flex flex-col gap-1.5">
            <SectionLabel>Provider Health</SectionLabel>
            <div className="flex flex-wrap gap-1">
              {snapshot.providerHealth.map((provider) => (
                <Badge
                  key={provider.providerId}
                  className="font-mono text-[10px]"
                  variant="outline"
                >
                  {provider.displayName} · {provider.status}
                </Badge>
              ))}
            </div>
          </section>

          <section className="flex flex-col gap-1.5">
            <SectionLabel>Recent Errors</SectionLabel>
            {snapshot.recentErrors.length === 0 ? (
              <div className="text-[12px] text-[var(--fg-dim)]">no recent errors</div>
            ) : (
              <ul className="flex flex-col gap-1">
                {snapshot.recentErrors.slice(0, 3).map((error) => (
                  <li
                    className="rounded-[var(--radius-sm)] border border-rose-400/30 bg-rose-500/10 px-2 py-1.5 text-[11px] text-rose-100"
                    key={`${error.occurredAtMs.toString()}-${error.source}`}
                  >
                    <span className="text-rose-200">{error.source}</span> · {error.message}
                  </li>
                ))}
              </ul>
            )}
          </section>

          <div className="flex flex-wrap gap-1 text-[10px] uppercase tracking-[0.14em]">
            <QuickLink label="Run Tree" />
            <QuickLink label="Approval Inbox" />
            <QuickLink label="Work Inbox" />
          </div>

          {errorMessage ? (
            <div className="text-[11px] text-[var(--status-failed)]">stale · {errorMessage}</div>
          ) : null}
        </div>
      )}
    </section>
  );
}

function WorkflowStatusPanel({ status }: { status?: WorkflowStatusResult }) {
  if (!status?.loaded) {
    return (
      <div className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-2 text-[11px] text-[var(--fg-dim)]">
        workflow not loaded · background orchestrator idle
      </div>
    );
  }

  const reloadStatus = status.lastReload?.status ?? "unknown";
  return (
    <div className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-2 text-[11px] text-[var(--fg)]">
      <div>{status.loaded.name}</div>
      <div className="text-[var(--fg-dim)]">
        {status.loaded.sourceKind} · {status.loaded.runtimeProfileCount} capsule profiles
      </div>
      <Badge
        className="mt-1 font-mono text-[10px]"
        variant={reloadStatus === "failed" ? "destructive" : "outline"}
      >
        reload {reloadStatus}
      </Badge>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[var(--radius-sm)] border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-1.5">
      <div className="text-[10px] uppercase tracking-[0.16em] text-[var(--fg-dim)]">{label}</div>
      <div className="text-[13px] text-[var(--fg)]">{value}</div>
    </div>
  );
}

function formatOptionalBigInt(value: bigint | null | undefined): string {
  return value?.toString() ?? "unknown";
}

function SectionLabel({ children }: { children: string }) {
  return (
    <div className="text-[10px] uppercase tracking-[0.18em] text-[var(--fg-mute)]">{children}</div>
  );
}

function QuickLink({ label }: { label: string }) {
  return (
    <span className="rounded border border-[var(--border)] px-1.5 py-1 text-[var(--fg-dim)]">
      {label}
    </span>
  );
}

function formatDuration(ms: bigint): string {
  const totalSeconds = Number(ms / 1000n);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}m ${seconds}s`;
}

function formatBoolean(value: boolean): string {
  return value ? "yes" : "no";
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
