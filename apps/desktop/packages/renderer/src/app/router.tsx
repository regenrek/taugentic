/**
 * Canonical workspace route surface.
 *
 * Hard-cut: the tabbed shell (overview/sessions/runs/approvals/artifacts) has
 * been retired in favor of a single unified master-detail workspace. The
 * renderer now exposes exactly one route id so legacy route-switching
 * machinery collapses into a no-op.
 */
export type AppRouteId = "workspace";
