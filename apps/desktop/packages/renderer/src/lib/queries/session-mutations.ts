import { useMutation, useQueryClient } from "@tanstack/react-query";

import type {
  ApprovalDecision,
  ApprovalId,
  AgentRuntimeModelId,
  RunId,
  RunRecord,
  RunSummary,
  SessionId,
} from "@taugentic/desktop-shared";

import { decideApproval, forkRun, startRun } from "@/lib/ipc/api";

import { queryKeys, sessionOverviewRootKey } from "./keys";

export interface StartRunVariables {
  modelId?: AgentRuntimeModelId | null;
  objective: string;
  recipeId?: string | null;
}

export function useStartRunMutation(sessionId: SessionId) {
  const qc = useQueryClient();
  return useMutation<RunSummary, Error, StartRunVariables>({
    mutationFn: ({ modelId = null, objective, recipeId = null }) =>
      startRun(sessionId, {
        modelId,
        objective,
        recipeId,
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.sessionRuns(sessionId) });
      void qc.invalidateQueries({ queryKey: queryKeys.sessionNativeRunsRoot(sessionId) });
      void qc.invalidateQueries({ queryKey: sessionOverviewRootKey });
    },
  });
}

export interface ForkRunVariables {
  objective?: string | null;
  parentEventSeq: bigint;
  parentRunId: RunId;
}

export function useForkRunMutation(sessionId: SessionId) {
  const qc = useQueryClient();
  return useMutation<RunRecord, Error, ForkRunVariables>({
    mutationFn: ({ objective = null, parentEventSeq, parentRunId }) =>
      forkRun(sessionId, {
        objective,
        parentEventSeq,
        parentRunId,
        sessionId,
      }).then((result) => result.run),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.sessionRuns(sessionId) });
      void qc.invalidateQueries({ queryKey: queryKeys.sessionNativeRunsRoot(sessionId) });
      void qc.invalidateQueries({ queryKey: sessionOverviewRootKey });
    },
  });
}

export interface DecideApprovalVariables {
  approvalId: ApprovalId;
  decision: ApprovalDecision;
}

export function useDecideApprovalMutation(sessionId: SessionId) {
  const qc = useQueryClient();
  return useMutation<RunSummary, Error, DecideApprovalVariables>({
    mutationFn: ({ approvalId, decision }) => decideApproval(sessionId, approvalId, decision),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.sessionApprovals(sessionId) });
      void qc.invalidateQueries({ queryKey: queryKeys.sessionRuns(sessionId) });
      void qc.invalidateQueries({ queryKey: queryKeys.sessionNativeRunsRoot(sessionId) });
      void qc.invalidateQueries({ queryKey: sessionOverviewRootKey });
    },
  });
}
