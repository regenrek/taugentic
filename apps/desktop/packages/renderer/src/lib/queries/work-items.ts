import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type {
  SessionId,
  WorkItemDismissParams,
  WorkItemDismissResult,
  WorkItemListResult,
  WorkItemRefreshParams,
  WorkItemTriggerParams,
  WorkItemTriggerResult,
} from "@taugentic/desktop-shared";

import { dismissWorkItem, listWorkItems, refreshWorkItems, triggerWorkItem } from "@/lib/ipc/api";

import { queryKeys, sessionOverviewRootKey } from "./keys";

export function useWorkItemsQuery() {
  return useQuery<WorkItemListResult, Error>({
    queryKey: queryKeys.workItems,
    queryFn: () => listWorkItems(),
  });
}

export function useRefreshWorkItemsMutation() {
  const qc = useQueryClient();
  return useMutation<WorkItemListResult, Error, WorkItemRefreshParams | undefined>({
    mutationFn: (params = {}) => refreshWorkItems(params),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.workItems });
    },
  });
}

export function useDismissWorkItemMutation() {
  const qc = useQueryClient();
  return useMutation<WorkItemDismissResult, Error, WorkItemDismissParams>({
    mutationFn: (params) => dismissWorkItem(params),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.workItems });
    },
  });
}

export function useTriggerWorkItemMutation(sessionId: SessionId | null) {
  const qc = useQueryClient();
  return useMutation<WorkItemTriggerResult, Error, WorkItemTriggerParams>({
    mutationFn: (params) => {
      if (sessionId === null) {
        throw new Error("Select a session before triggering a work item.");
      }
      return triggerWorkItem(sessionId, params);
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.workItems });
      void qc.invalidateQueries({ queryKey: sessionOverviewRootKey });
      if (sessionId !== null) {
        void qc.invalidateQueries({ queryKey: queryKeys.sessionRuns(sessionId) });
        void qc.invalidateQueries({ queryKey: queryKeys.sessionNativeRunsRoot(sessionId) });
      }
    },
  });
}
