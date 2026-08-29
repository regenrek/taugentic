import { useMutation, useQuery } from "@tanstack/react-query"
import { useState } from "react"

import type { PluginCapability, PluginInspection, PluginInstallation } from "@taugentic/desktop-protocol"

import type { PluginDesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { desktopQueryClient } from "../../platform/daemon/query-client.js"
import { pluginsQuery, pluginsQueryKey, type PluginsRuntime } from "../../platform/daemon/plugins-query.js"

type PluginCommands = Pick<PluginDesktopRuntime, "inspectPluginPackage" | "installPluginPackage" | "uninstallPlugin">

function errorMessage(error: unknown): string {
  return error instanceof Error && error.message ? error.message : "Plugins could not be updated."
}

export type PluginsState = {
  installations: readonly PluginInstallation[]
  inspection?: PluginInspection
  grantedCapabilities: readonly PluginCapability[]
  loading: boolean
  busy: boolean
  error?: string
  mutationError?: string
  inspect(sourcePath: string): void
  setGranted(capability: PluginCapability, granted: boolean): void
  install(): void
  uninstall(installation: PluginInstallation): void
  clearInspection(): void
}

/** Owns transient install review state and the only Plugin mutation invalidation path. */
export function usePlugins(input: { runtime: PluginsRuntime & PluginCommands; enabled: boolean }): PluginsState {
  const [sourcePath, setSourcePath] = useState<string>()
  const [inspection, setInspection] = useState<PluginInspection>()
  const [grantedCapabilities, setGrantedCapabilities] = useState<PluginCapability[]>([])
  const query = useQuery({ ...pluginsQuery(input.runtime), enabled: input.enabled })
  const inspectMutation = useMutation({
    mutationFn: (path: string) => input.runtime.inspectPluginPackage({ sourcePath: path }),
    onSuccess: (result, path) => {
      setSourcePath(path)
      setInspection(result)
      setGrantedCapabilities([])
    },
  })
  const installMutation = useMutation({
    mutationFn: (request: { sourcePath: string; inspection: PluginInspection; grantedCapabilities: PluginCapability[] }) => input.runtime.installPluginPackage(request),
    onSuccess: () => {
      setSourcePath(undefined)
      setInspection(undefined)
      setGrantedCapabilities([])
      void desktopQueryClient.invalidateQueries({ queryKey: pluginsQueryKey })
    },
  })
  const uninstallMutation = useMutation({
    mutationFn: (installation: PluginInstallation) => input.runtime.uninstallPlugin({
      pluginId: installation.pluginId,
      version: installation.version,
      digestSha256: installation.digestSha256,
    }),
    onSuccess: () => void desktopQueryClient.invalidateQueries({ queryKey: pluginsQueryKey }),
  })
  const busy = inspectMutation.isPending || installMutation.isPending || uninstallMutation.isPending
  const mutationError = inspectMutation.error ?? installMutation.error ?? uninstallMutation.error

  return {
    installations: input.enabled ? query.data?.installations ?? [] : [],
    inspection,
    grantedCapabilities,
    loading: input.enabled && query.isLoading,
    busy,
    error: input.enabled && query.isError ? "Plugins could not be loaded." : undefined,
    mutationError: mutationError ? errorMessage(mutationError) : undefined,
    inspect: (path) => {
      if (input.enabled && !busy && path) inspectMutation.mutate(path)
    },
    setGranted: (capability, granted) => {
      setGrantedCapabilities((current) => granted
        ? current.includes(capability) ? current : [...current, capability]
        : current.filter((item) => item !== capability))
    },
    install: () => {
      if (!input.enabled || busy || !sourcePath || !inspection) return
      installMutation.mutate({ sourcePath, inspection, grantedCapabilities })
    },
    uninstall: (installation) => {
      if (input.enabled && !busy) uninstallMutation.mutate(installation)
    },
    clearInspection: () => {
      if (!busy) {
        setSourcePath(undefined)
        setInspection(undefined)
        setGrantedCapabilities([])
      }
    },
  }
}
