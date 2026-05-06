import type {
  AgentRuntimeStrategyId,
  AgentRuntimeStrategyInfo,
  AgentRuntimeSnapshot,
  RuntimeProfileSummary,
} from "@taugentic/desktop-shared";

/**
 * Pure selectors + filters for the runtime picker.
 *
 * Extracted so the filter/search/grouping UX is unit-tested without
 * driving the whole React panel. The existing AgentRuntimeSnapshot
 * shape is the single source of truth — these helpers never fabricate
 * state the daemon did not expose.
 */

export type RuntimeReadinessFilter = "any" | "readyOnly";

export interface RuntimeProfileFilterOptions {
  readonly query: string;
  readonly readiness: RuntimeReadinessFilter;
}

const RUNTIME_READINESS_TONE: Record<
  NonNullable<AgentRuntimeStrategyInfo["health"]["status"]>,
  "ready" | "degraded" | "unavailable" | "unknown"
> = {
  ready: "ready",
  degraded: "degraded",
  unavailable: "unavailable",
  unknown: "unknown",
};

/**
 * Filters runtime profiles by free-text query and readiness gate.
 *
 * The query matches case-insensitively against `displayName`,
 * `providerId`, and the provider display name (when resolvable).
 */
export function filterRuntimeProfiles(
  snapshot: AgentRuntimeSnapshot | undefined,
  options: RuntimeProfileFilterOptions,
): RuntimeProfileSummary[] {
  const profiles = snapshot?.runtimeProfiles ?? [];
  if (profiles.length === 0) {
    return [];
  }
  const providers = snapshot?.providers ?? [];
  const normalizedQuery = options.query.trim().toLowerCase();

  return profiles.filter((profile) => {
    if (options.readiness === "readyOnly") {
      const provider = providerById(providers, profile.providerId);
      if (provider?.health.status !== "ready") {
        return false;
      }
    }
    if (normalizedQuery.length === 0) {
      return true;
    }
    const provider = providerById(providers, profile.providerId);
    const haystack = [profile.displayName, profile.providerId, provider?.displayName ?? ""]
      .join(" ")
      .toLowerCase();
    return haystack.includes(normalizedQuery);
  });
}

/**
 * Groups runtime profiles by provider id, preserving the snapshot's
 * provider ordering and placing unknown-provider profiles at the end.
 */
export function groupRuntimeProfilesByProvider(
  snapshot: AgentRuntimeSnapshot | undefined,
  profiles: readonly RuntimeProfileSummary[],
): Array<{
  providerId: AgentRuntimeStrategyId;
  providerDisplayName: string;
  providerHealthStatus: AgentRuntimeStrategyInfo["health"]["status"];
  profiles: RuntimeProfileSummary[];
}> {
  const providers = snapshot?.providers ?? [];
  const orderedIds = providers.map((provider) => provider.id);
  const groupMap = new Map<AgentRuntimeStrategyId, RuntimeProfileSummary[]>();

  for (const profile of profiles) {
    const bucket = groupMap.get(profile.providerId) ?? [];
    bucket.push(profile);
    groupMap.set(profile.providerId, bucket);
  }

  const groups: Array<{
    providerId: AgentRuntimeStrategyId;
    providerDisplayName: string;
    providerHealthStatus: AgentRuntimeStrategyInfo["health"]["status"];
    profiles: RuntimeProfileSummary[];
  }> = [];

  for (const providerId of orderedIds) {
    const bucket = groupMap.get(providerId);
    if (bucket === undefined) {
      continue;
    }
    const provider = providerById(providers, providerId);
    groups.push({
      providerId,
      providerDisplayName: provider?.displayName ?? providerId,
      providerHealthStatus: provider?.health.status ?? "unknown",
      profiles: bucket,
    });
    groupMap.delete(providerId);
  }

  for (const [providerId, bucket] of groupMap) {
    groups.push({
      providerId,
      providerDisplayName: providerId,
      providerHealthStatus: "unknown",
      profiles: bucket,
    });
  }

  return groups;
}

/**
 * Human-readable description for a runtime provider's current health tone,
 * preserving any daemon-supplied `health.message` where present.
 */
export function describeProviderHealth(provider: AgentRuntimeStrategyInfo | null): {
  tone: "ready" | "degraded" | "unavailable" | "unknown" | "missing";
  label: string;
  detail: string | null;
} {
  if (provider === null) {
    return { tone: "missing", label: "no provider selected", detail: null };
  }
  const tone = RUNTIME_READINESS_TONE[provider.health.status] ?? "unknown";
  const label = provider.health.status;
  const detail = provider.health.message?.trim() ?? null;
  return { tone, label, detail: detail && detail.length > 0 ? detail : null };
}

function providerById(
  providers: readonly AgentRuntimeStrategyInfo[],
  providerId: AgentRuntimeStrategyId,
): AgentRuntimeStrategyInfo | null {
  return providers.find((provider) => provider.id === providerId) ?? null;
}
