import type {
  AgentRuntimeSnapshot,
  AgentRuntimeStrategyHealthStatus,
  AgentRuntimeStrategyInfo,
} from "@taugentic/desktop-shared";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusDot, type StatusTone } from "@/components/ui/status-dot";
import { useAgentRuntimeQuery } from "@/lib/queries/agent-runtime";

/**
 * Right-rail inspector card: per-provider runtime health.
 *
 * Reads via existing query owner only — no new domain owner is introduced.
 */
export function ProviderHealthCard() {
  const runtime = useAgentRuntimeQuery();
  return (
    <ProviderHealthCardView
      errorMessage={runtime.error ? toErrorMessage(runtime.error) : null}
      isLoading={runtime.isLoading}
      snapshot={runtime.data}
    />
  );
}

export interface ProviderHealthCardViewProps {
  errorMessage: string | null;
  isLoading: boolean;
  snapshot: AgentRuntimeSnapshot | undefined;
}

export function ProviderHealthCardView({
  errorMessage,
  isLoading,
  snapshot,
}: ProviderHealthCardViewProps) {
  const providers: AgentRuntimeStrategyInfo[] = snapshot?.providers ?? [];

  return (
    <Card
      aria-label="Provider health"
      className="border-x-0 border-t-0 rounded-none"
      data-feature="provider-health-card"
    >
      <CardHeader className="px-3 pt-3 pb-2">
        <CardTitle className="text-[10px] uppercase tracking-[0.18em] text-[var(--fg-mute)]">
          PROVIDER HEALTH
        </CardTitle>
      </CardHeader>
      <CardContent className="px-3 pb-3">
        {isLoading && snapshot === undefined ? (
          <div className="text-[12px] text-[var(--fg-dim)]" data-state="loading">
            Loading providers…
          </div>
        ) : errorMessage !== null && snapshot === undefined ? (
          <div className="text-[12px] text-[var(--status-failed)]" data-state="error">
            error: {errorMessage}
          </div>
        ) : providers.length === 0 ? (
          <div className="text-[12px] text-[var(--fg-dim)]" data-state="empty">
            no providers
          </div>
        ) : (
          <ul className="flex flex-col gap-1.5" data-state="ready">
            {providers.map((provider) => (
              <ProviderHealthRow key={provider.id} provider={provider} />
            ))}
          </ul>
        )}
        {errorMessage !== null && snapshot !== undefined ? (
          <div className="mt-2 text-[11px] text-[var(--status-failed)]" data-state="stale">
            stale · {errorMessage}
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

function ProviderHealthRow({ provider }: { provider: AgentRuntimeStrategyInfo }) {
  const tone = providerHealthTone(provider.health.status);
  const modelCount = provider.models?.length ?? 0;
  const detail = formatProviderHealthDetail(provider);

  return (
    <li
      className="flex items-center justify-between gap-2 border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-1.5 font-[var(--font-mono)] text-[12px] text-[var(--fg)]"
      data-provider-id={provider.id}
      data-provider-status={provider.health.status}
      data-tone={tone}
      title={detail}
    >
      <div className="flex min-w-0 items-center gap-2">
        <StatusDot tone={tone} />
        <span className="truncate">{provider.displayName}</span>
        <span className="text-[var(--fg-mute)]">·</span>
        <span className="text-[10px] uppercase tracking-[0.16em] text-[var(--fg-dim)]">
          {provider.id}
        </span>
      </div>
      <div className="flex shrink-0 items-center gap-2 text-[10px] uppercase tracking-[0.16em] text-[var(--fg-dim)]">
        <span data-provider-model-count>
          {modelCount} model{modelCount === 1 ? "" : "s"}
        </span>
        <span className="text-[var(--fg-mute)]">·</span>
        <span data-provider-status-label>{provider.health.status}</span>
      </div>
    </li>
  );
}

function providerHealthTone(status: AgentRuntimeStrategyHealthStatus): StatusTone {
  switch (status) {
    case "ready":
      return "active";
    case "degraded":
      return "waiting";
    case "unavailable":
      return "failed";
    case "unknown":
    default:
      return "idle";
  }
}

function formatProviderHealthDetail(provider: AgentRuntimeStrategyInfo): string {
  const base = `${provider.displayName} (${provider.id}) · ${provider.health.status}`;
  if (provider.health.message) {
    return `${base} · ${provider.health.message}`;
  }
  return base;
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
