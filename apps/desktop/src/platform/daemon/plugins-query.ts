import { queryOptions } from "@tanstack/react-query"

import type { ListPluginInstallationsResult } from "@taugentic/desktop-protocol"

import type { PluginDesktopRuntime } from "./desktop-runtime.js"

export type PluginsRuntime = Pick<PluginDesktopRuntime, "listPluginInstallations">

export const pluginsQueryKey = ["daemon", "plugins"] as const

/** The sole desktop cache for principal-scoped Plugin installations. */
export function pluginsQuery(runtime: PluginsRuntime) {
  return queryOptions({
    queryKey: pluginsQueryKey,
    queryFn: (): Promise<ListPluginInstallationsResult> => runtime.listPluginInstallations(),
  })
}
