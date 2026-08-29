import { queryOptions } from "@tanstack/react-query"

import type { DaemonDiagnostics } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"

export const diagnosticsQueryKey = ["daemon", "diagnostics"] as const

/** The sole desktop query for the daemon-owned diagnostics snapshot. */
export function diagnosticsQuery(runtime: DesktopRuntime) {
  return queryOptions({
    queryKey: diagnosticsQueryKey,
    queryFn: (): Promise<DaemonDiagnostics> => runtime.diagnosticsSnapshot(),
    retry: false,
    refetchInterval: false,
    refetchOnReconnect: false,
    refetchOnWindowFocus: false,
  })
}
