import { useQuery, useQueryClient } from "@tanstack/react-query"

import type { ApprovalDecision, ApprovalId, ApprovalRequest, SessionId } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { approvalsInboxQuery, runActivityQueryRoot } from "../../platform/daemon/run-activity-query.js"

const NO_SESSION = "session-not-selected" as SessionId

/** The one desktop cache and decision owner for daemon-owned pending approvals. */
export function useApprovalsInbox(input: { runtime: DesktopRuntime; sessionId?: SessionId; enabled: boolean }) {
  const sessionId = input.sessionId ?? NO_SESSION
  const queryClient = useQueryClient()
  const enabled = input.enabled && Boolean(input.sessionId)
  const approvals = useQuery({ ...approvalsInboxQuery(input.runtime, sessionId), enabled })
  const decide = async (approvalId: ApprovalId, decision: ApprovalDecision) => {
    await input.runtime.bridge.decideApproval(JSON.stringify({ approvalId, decision }))
    await queryClient.invalidateQueries({ queryKey: [...runActivityQueryRoot, sessionId] })
  }
  return {
    approvals: approvals.data?.items ?? [] as readonly ApprovalRequest[],
    loading: approvals.isLoading,
    error: approvals.isError ? "Approvals could not be loaded." : undefined,
    decide,
  }
}

export type ApprovalsInboxState = ReturnType<typeof useApprovalsInbox>
