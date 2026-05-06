import type { RunStatus } from "@taugentic/desktop-shared";

import type { BadgeProps } from "@/components/ui/badge";
import type { StatusTone } from "@/components/ui/status-dot";

export type RunPresentationStatus = RunStatus | "contractViolation" | "quarantined";

export interface RunStatusPresentation {
  badgeVariant: BadgeProps["variant"];
  label: string;
  tone: StatusTone;
}

export function getRunStatusPresentation(status: RunPresentationStatus): RunStatusPresentation {
  switch (status) {
    case "queued":
      return { badgeVariant: "outline", label: "queued", tone: "idle" };
    case "running":
      return { badgeVariant: "secondary", label: "running", tone: "active" };
    case "waitingForApproval":
      return { badgeVariant: "outline", label: "waiting", tone: "waiting" };
    case "completed":
      return { badgeVariant: "accent", label: "completed", tone: "completed" };
    case "failed":
      return { badgeVariant: "destructive", label: "failed", tone: "failed" };
    case "budgetExceeded":
      return { badgeVariant: "destructive", label: "budget exceeded", tone: "failed" };
    case "cancelled":
      return { badgeVariant: "outline", label: "cancelled", tone: "cancelled" };
    case "contractViolation":
      return { badgeVariant: "destructive", label: "contract violation", tone: "failed" };
    case "quarantined":
      return { badgeVariant: "destructive", label: "quarantined", tone: "failed" };
  }
}

export function runStatusTone(status: RunPresentationStatus): StatusTone {
  return getRunStatusPresentation(status).tone;
}
