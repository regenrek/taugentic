import { useQuery } from "@tanstack/react-query";

import type { DaemonDiagnostics } from "@taugentic/desktop-shared";

import { getDaemonDiagnostics } from "@/lib/ipc/api";

import { queryKeys } from "./keys";
import { SESSION_QUERY_POLL_INTERVAL_MS, type SessionQueryView } from "./session-queries";

export function useDaemonDiagnosticsQuery(): SessionQueryView<DaemonDiagnostics> {
  const query = useQuery({
    queryKey: queryKeys.daemon.diagnostics,
    queryFn: () => getDaemonDiagnostics(),
    refetchInterval: SESSION_QUERY_POLL_INTERVAL_MS,
  });
  return {
    data: query.data,
    error: query.error,
    isFetching: query.isFetching,
    isLoading: query.isLoading,
    refetch: query.refetch,
  };
}
