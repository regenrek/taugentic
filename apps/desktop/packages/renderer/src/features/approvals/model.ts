import type { ApprovalDecision, ApprovalId } from "@taugentic/desktop-shared";

import type { SessionApprovalSnapshot } from "./connection";
import type { SessionApprovalState } from "./stream-state";

export interface SessionApprovalViewState extends SessionApprovalState {
  commandErrorMessage: string | null;
  pendingApprovalId: ApprovalId | null;
  pendingDecision: ApprovalDecision | null;
}

export function selectSessionApprovalViewState(
  snapshot: SessionApprovalSnapshot,
): SessionApprovalViewState {
  return {
    commandErrorMessage: snapshot.context.commandErrorMessage,
    errorMessage: snapshot.context.errorMessage,
    lastSequence: snapshot.context.lastSequence,
    pendingApprovalId: snapshot.context.pendingApprovalId,
    pendingDecision: snapshot.context.pendingDecision,
    sessionId: snapshot.context.sessionId,
    streamStatus: snapshot.context.streamStatus,
  };
}
