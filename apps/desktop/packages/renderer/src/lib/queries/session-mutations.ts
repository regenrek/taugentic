import { useMutation, useQueryClient } from "@tanstack/react-query";

import type {
  ApprovalDecision,
  ApprovalId,
  AgentRuntimeModelId,
  RunId,
  RunRecord,
  RunSummary,
  SessionId,
  SessionSummary,
  WorkspaceSelector,
} from "@taugentic/desktop-shared";

import { decideApproval, forkRun, openSession, startRun } from "@/lib/ipc/api";

import { queryKeys, sessionListRootKey, sessionOverviewRootKey } from "./keys";

export function useOpenSessionMutation() {
  const qc = useQueryClient();
  return useMutation<SessionSummary, Error, { title: string; workspace: WorkspaceSelector }>({
    mutationFn: ({ title, workspace }) => openSession(title, workspace),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: sessionListRootKey });
      void qc.invalidateQueries({ queryKey: sessionOverviewRootKey });
    },
  });
}

export interface StartRunVariables {
  modelId?: AgentRuntimeModelId | null;
  objective: string;
  recipeId?: string | null;
  sandboxProfile?: string | null;
}

export function useStartRunMutation(sessionId: SessionId) {
  const qc = useQueryClient();
  return useMutation<RunSummary, Error, StartRunVariables>({
    mutationFn: ({ modelId = null, objective, recipeId = null, sandboxProfile = null }) =>
      startRun(sessionId, {
        modelId,
        objective,
        recipeId,
        sandboxProfile,
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
