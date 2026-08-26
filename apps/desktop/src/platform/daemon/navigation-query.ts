import { QueryClient, queryOptions } from "@tanstack/react-query"

import type { NavigationSnapshot } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"
import { decodeProtocolJson } from "./protocol-json.js"

export const navigationQueryKey = ["daemon", "navigation"] as const

export const navigationQueryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: Infinity } },
})

/** The only TypeScript cache for daemon-owned navigation rows. */
export function navigationQuery(runtime: DesktopRuntime, search?: string) {
  return queryOptions({
    queryKey: search ? [...navigationQueryKey, search] : navigationQueryKey,
    queryFn: async (): Promise<NavigationSnapshot> => decodeProtocolJson(await runtime.bridge.navigationSnapshot(search)),
  })
}

export function invalidateNavigation(): Promise<void> {
  return navigationQueryClient.invalidateQueries({ queryKey: navigationQueryKey })
}
