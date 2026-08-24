import { useState } from "react";

import { useMutation, useQueryClient } from "@tanstack/react-query";

import type { Workspace } from "@taugentic/desktop-shared";

import { openWorkspace, pickWorkspaceFolder } from "@/lib/ipc/api";
import { queryKeys } from "@/lib/queries/keys";

type WorkspacePickerResult =
  | { status: "cancelled" }
  | { status: "opened"; workspace: Workspace }
  | { status: "trustRequired"; path: string };

export function useWorkspacePicker() {
  const [trustPath, setTrustPath] = useState<string | null>(null);
  const qc = useQueryClient();
  const openWorkspaceMutation = useMutation({
    mutationFn: (params: { path: string; trustAcknowledged: boolean }) => openWorkspace(params),
    onSuccess: (result) => {
      if (result.status === "opened") {
        void qc.invalidateQueries({ queryKey: queryKeys.workspaces });
      }
    },
  });

  return {
    confirmTrust: async (): Promise<WorkspacePickerResult> => {
      if (trustPath === null) {
        return { status: "cancelled" };
      }
      try {
        const result = await openWorkspaceMutation.mutateAsync({
          path: trustPath,
          trustAcknowledged: true,
        });
        if (result.status !== "opened") {
          throw new Error("Trusted workspace acknowledgement was rejected by the daemon.");
        }
        return { status: "opened", workspace: result.workspace };
      } finally {
        setTrustPath(null);
      }
    },
    cancelTrust: () => setTrustPath(null),
    isPending: openWorkspaceMutation.isPending,
    pickWorkspace: async (): Promise<WorkspacePickerResult> => {
      const path = await pickWorkspaceFolder();
      if (path === null) {
        return { status: "cancelled" };
      }
      const result = await openWorkspaceMutation.mutateAsync({
        path,
        trustAcknowledged: false,
      });
      if (result.status === "trustRequired") {
        setTrustPath(result.path);
        return result;
      }
      return { status: "opened", workspace: result.workspace };
    },
    trustPath,
  };
}
