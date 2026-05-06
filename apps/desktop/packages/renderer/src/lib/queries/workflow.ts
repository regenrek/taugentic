import { useQuery } from "@tanstack/react-query";

import type { WorkflowStatusResult } from "@taugentic/desktop-shared";

import { getWorkflowStatus } from "@/lib/ipc/api";

import { queryKeys } from "./keys";

export function useWorkflowStatusQuery() {
  return useQuery<WorkflowStatusResult, Error>({
    queryKey: queryKeys.workflow.status,
    queryFn: () => getWorkflowStatus(),
  });
}
