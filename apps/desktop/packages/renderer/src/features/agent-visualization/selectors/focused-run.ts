/*
 * Tiny selectors over canonical session queries.
 *
 * No new daemon-derived owner -- these helpers ONLY transform data already
 * exposed by `@/lib/queries/session-queries`. Components inside
 * agent-visualization/ should consume these helpers instead of duplicating
 * sort/pick logic, so the panel stays presentation-only.
 */

import type { PublicActivityPageItem, RunSummary } from "@taugentic/desktop-shared";

/**
 * Picks the most recent run from a `useSessionRunsQuery` result.
 *
 * The daemon returns runs ordered newest-first; this helper centralizes
 * that contract and tolerates an undefined input.
 */
export function pickLatestRun(runs: readonly RunSummary[] | undefined): RunSummary | null {
  if (runs === undefined || runs.length === 0) {
    return null;
  }
  return runs[0] ?? null;
}

/**
 * Returns the most recent N activity items in descending sequence order
 * (newest first). The underlying query already returns recency-sorted pages
 * but the order is not guaranteed across sources, so we sort defensively
 * by `cursor.sequence` here (the canonical recency field on
 * `PublicActivityPageItem`).
 */
export function pickRecentActivity(
  items: readonly PublicActivityPageItem[] | undefined,
  limit: number,
): PublicActivityPageItem[] {
  if (items === undefined || items.length === 0 || limit <= 0) {
    return [];
  }
  const sorted = [...items].sort(compareActivityDescending);
  if (sorted.length <= limit) {
    return sorted;
  }
  return sorted.slice(0, limit);
}

function compareActivityDescending(a: PublicActivityPageItem, b: PublicActivityPageItem): number {
  const aSeq = sequenceToNumber(a.cursor.sequence);
  const bSeq = sequenceToNumber(b.cursor.sequence);
  if (aSeq === bSeq) {
    return 0;
  }
  return aSeq < bSeq ? 1 : -1;
}

function sequenceToNumber(value: bigint | number): number {
  return typeof value === "bigint" ? Number(value) : value;
}
