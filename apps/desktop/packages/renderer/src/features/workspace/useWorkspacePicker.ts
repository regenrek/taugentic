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
    mutationFn: (params: { path: string; trustAcknowledged: boolean }) =>
      openWorkspace(params).then((result) => result.workspace),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.workspaces });
    },
  });

  return {
    confirmTrust: async (): Promise<WorkspacePickerResult> => {
      if (trustPath === null) {
        return { status: "cancelled" };
      }
      try {
        const workspace = await openWorkspaceMutation.mutateAsync({
          path: trustPath,
          trustAcknowledged: true,
        });
        return { status: "opened", workspace };
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
      try {
        const workspace = await openWorkspaceMutation.mutateAsync({
          path,
          trustAcknowledged: false,
        });
        return { status: "opened", workspace };
      } catch (error) {
        if (isWorkspaceTrustRequired(error)) {
          setTrustPath(path);
          return { status: "trustRequired", path };
        }
        throw error;
      }
    },
    trustPath,
  };
}

function isWorkspaceTrustRequired(error: unknown): boolean {
  const data =
    error instanceof Error && "data" in error ? (error as { data?: unknown }).data : null;
  if (isRecord(data) && data.code === "WorkspaceTrustRequired") {
    return true;
  }
  return error instanceof Error && error.message.includes("WorkspaceTrustRequired");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
