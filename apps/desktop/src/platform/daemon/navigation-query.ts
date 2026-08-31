import { queryOptions } from "@tanstack/react-query"

import type { NavigationSnapshot } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"
import { decodeProtocolJson } from "./protocol-json.js"
import { desktopQueryClient } from "./query-client.js"

export const navigationQueryKey = ["daemon", "navigation"] as const

/** The only TypeScript cache for daemon-owned navigation rows. */
export function navigationQuery(runtime: DesktopRuntime, search?: string) {
  return queryOptions({
    queryKey: search ? [...navigationQueryKey, search] : navigationQueryKey,
    queryFn: async (): Promise<NavigationSnapshot> => decodeProtocolJson(await runtime.bridge.navigationSnapshot(search)),
  })
}

export function invalidateNavigation(): Promise<void> {
  return desktopQueryClient.invalidateQueries({ queryKey: navigationQueryKey })
}

/** Refresh and publish the authoritative daemon navigation projection. */
export async function refreshNavigationSnapshot(runtime: DesktopRuntime): Promise<NavigationSnapshot> {
  await invalidateNavigation()
  return desktopQueryClient.fetchQuery(navigationQuery(runtime))
}

/** Publish an authoritative daemon mutation result into the sole navigation projection. */
export function updateNavigationSnapshot(snapshot: NavigationSnapshot): void {
  desktopQueryClient.setQueryData(navigationQueryKey, snapshot)
}
