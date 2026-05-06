/*
 * MetricsSection (extracted for FocusedRunTabs.MetricsTab).
 *
 * The pre-existing SessionDetailSurface had no dedicated metrics
 * sub-component. To keep the visualization tabs as thin wrappers (no
 * re-implementation in `agent-visualization/`), the minimal aggregate
 * view is extracted into the canonical session-detail package and
 * consumed verbatim by MetricsTab.
 *
 * No new daemon-derived owner is added: this section reads ONLY from the
 * existing TanStack Query hooks already used by the other sections.
 */

import type { SessionId } from "@taugentic/desktop-shared";

import {
  useSessionActivityQuery,
  useSessionApprovalsQuery,
  useSessionArtifactsQuery,
  useSessionRunsQuery,
} from "@/lib/queries/session-queries";

import { describeRunStatus, splitLatestAndOlderRuns } from "./formatters";
import { SectionHeader } from "./section-header";

export interface MetricsSectionProps {
  sessionId: SessionId;
}

export function MetricsSection({ sessionId }: MetricsSectionProps) {
  const runsQuery = useSessionRunsQuery(sessionId);
  const activityQuery = useSessionActivityQuery(sessionId);
  const approvalsQuery = useSessionApprovalsQuery(sessionId);
  const artifactsQuery = useSessionArtifactsQuery(sessionId);

  const runs = runsQuery.data ?? [];
  const activity = activityQuery.data ?? [];
  const approvals = approvalsQuery.data ?? [];
  const artifacts = artifactsQuery.data ?? [];

  const { latest } = splitLatestAndOlderRuns(runs);
  const latestStatus = latest === null ? "—" : describeRunStatus(latest.status);

  const errorMessage = firstErrorMessage([
    runsQuery.error,
    activityQuery.error,
    approvalsQuery.error,
    artifactsQuery.error,
  ]);
  const hasLoaded =
    runsQuery.data !== undefined &&
    activityQuery.data !== undefined &&
    approvalsQuery.data !== undefined &&
    artifactsQuery.data !== undefined;
  const pending =
    runsQuery.isFetching ||
    activityQuery.isFetching ||
    approvalsQuery.isFetching ||
    artifactsQuery.isFetching;

  const items: Array<{ label: string; value: string }> = [
    { label: "runs", value: String(runs.length) },
    { label: "activity", value: String(activity.length) },
    { label: "pending approvals", value: String(approvals.length) },
    { label: "artifacts", value: String(artifacts.length) },
    { label: "latest status", value: latestStatus },
  ];

  return (
    <section className="flex flex-col gap-2 px-3 py-3" data-section="metrics">
      <SectionHeader
        count={items.length}
        errorMessage={errorMessage}
        hasLoaded={hasLoaded}
        label="metrics"
        pending={pending}
      />
      <dl className="grid grid-cols-2 gap-x-4 gap-y-1 font-[var(--font-mono)] text-[12px] text-[var(--fg)] sm:grid-cols-3">
        {items.map((item) => (
          <div className="flex items-baseline gap-2" data-metric={item.label} key={item.label}>
            <dt className="text-[10px] uppercase tracking-[0.18em] text-[var(--fg-dim)]">
              {item.label}
            </dt>
            <dd className="truncate text-[var(--fg)]">{item.value}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

function firstErrorMessage(errors: readonly unknown[]): string | null {
  for (const error of errors) {
    if (error !== null && error !== undefined) {
      return toUnknownErrorMessage(error);
    }
  }
  return null;
}

function toUnknownErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (
    typeof error === "string" ||
    typeof error === "number" ||
    typeof error === "boolean" ||
    typeof error === "bigint"
  ) {
    return String(error);
  }
  return JSON.stringify(error) ?? "unknown error";
}
