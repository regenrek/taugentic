import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { QueryClientProvider } from "@tanstack/react-query"
import { describe, expect, it } from "bun:test"

import type { PluginInspection, PluginInstallation } from "@taugentic/desktop-protocol"

import { PluginsPanel } from "../src/features/plugins/plugins-panel.js"
import { usePlugins } from "../src/features/plugins/use-plugins.js"
import type { PluginDesktopRuntime } from "../src/platform/daemon/desktop-runtime.js"
import { desktopQueryClient } from "../src/platform/daemon/query-client.js"

type PluginsRuntime = PluginDesktopRuntime

const inspection: PluginInspection = {
  pluginId: "example.plugin" as PluginInspection["pluginId"],
  version: "1.2.3",
  digestSha256: "a".repeat(64),
  requestedCapabilities: ["workspaceRead", "network"],
}

function click(renderer: ReturnType<typeof createTestRoot>["renderer"], testId: string) {
  const element = renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id) ?? []
  renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0))
}

function Harness({ runtime }: { runtime: PluginsRuntime }) {
  const plugins = usePlugins({ runtime, enabled: true })
  return <div><div testId="inspect-fixture" accessibilityRole="button" accessibilityName="Inspect fixture" onClick={() => plugins.inspect("/transient/plugin-package")} style={{ width: 120, height: 28 }}><text>Inspect</text></div><PluginsPanel plugins={plugins} onClose={() => {}} /></div>
}

describe("M4 Plugins", () => {
  it("reviews immutable inspection metadata and sends an explicit empty grant set", async () => {
    const installs: unknown[] = []
    const runtime: PluginsRuntime = {
      async inspectPluginPackage() { return inspection },
      async installPluginPackage(request) { installs.push(request); return { installation: {} as PluginInstallation } },
      async listPluginInstallations() { return { installations: [] } },
      async uninstallPlugin() {},
    }
    const root = createTestRoot()
    try {
      desktopQueryClient.clear()
      root.render(<QueryClientProvider client={desktopQueryClient}><Harness runtime={runtime} /></QueryClientProvider>)
      await settle()
      click(root.renderer, "inspect-fixture")
      await settle()
      await settle()
      expect(root.renderer.findByTestId("plugin-inspection-version")).toBeDefined()
      expect(root.renderer.findByTestId("plugin-inspection-digest")).toBeDefined()
      click(root.renderer, "confirm-plugin-install")
      await settle()
      expect(installs).toEqual([{
        sourcePath: "/transient/plugin-package",
        inspection,
        grantedCapabilities: [],
      }])
    } finally {
      root.unmount()
      desktopQueryClient.clear()
    }
  })

  it("sends only the explicitly selected requested capability", async () => {
    const installs: unknown[] = []
    const runtime: PluginsRuntime = {
      async inspectPluginPackage() { return inspection },
      async installPluginPackage(request) { installs.push(request); return { installation: {} as PluginInstallation } },
      async listPluginInstallations() { return { installations: [] } },
      async uninstallPlugin() {},
    }
    const root = createTestRoot()
    try {
      desktopQueryClient.clear()
      root.render(<QueryClientProvider client={desktopQueryClient}><Harness runtime={runtime} /></QueryClientProvider>)
      await settle()
      click(root.renderer, "inspect-fixture")
      await settle()
      await settle()
      click(root.renderer, "plugin-capability-network")
      click(root.renderer, "confirm-plugin-install")
      await settle()
      expect(installs).toEqual([expect.objectContaining({ grantedCapabilities: ["network"] })])
    } finally {
      root.unmount()
      desktopQueryClient.clear()
    }
  })

  it("shows only authoritative Disabled state and uninstalls by exact immutable identity", async () => {
    const installation: PluginInstallation = {
      pluginId: "example.plugin" as PluginInstallation["pluginId"],
      version: "1.2.3",
      digestSha256: "b".repeat(64),
      requestedCapabilities: ["workspaceRead"],
      grantedCapabilities: [],
      lifecycleState: "disabled",
    }
    const samePluginDifferentPackage: PluginInstallation = {
      ...installation,
      version: "2.0.0",
      digestSha256: "c".repeat(64),
    }
    const uninstalls: unknown[] = []
    const runtime: PluginsRuntime = {
      async inspectPluginPackage() { return inspection },
      async installPluginPackage() { throw new Error("Not used.") },
      async listPluginInstallations() { return { installations: [installation, samePluginDifferentPackage] } },
      async uninstallPlugin(request) { uninstalls.push(request) },
    }
    const root = createTestRoot()
    try {
      desktopQueryClient.clear()
      root.render(<QueryClientProvider client={desktopQueryClient}><Harness runtime={runtime} /></QueryClientProvider>)
      await settle()
      await settle()
      const automation = root.renderer.getAutomationTree()
      expect(automation).toContain("Disabled")
      expect(automation).not.toContain("Activate")
      expect(automation).not.toContain("Enable")
      const immutableIdentity = `example.plugin-1.2.3-${"b".repeat(64)}`
      expect(root.renderer.findByTestId(`plugin-installation-${immutableIdentity}`)).toBeDefined()
      expect(root.renderer.findByTestId(`plugin-installation-example.plugin-2.0.0-${"c".repeat(64)}`)).toBeDefined()
      click(root.renderer, `uninstall-plugin-${immutableIdentity}`)
      await settle()
      expect(uninstalls).toEqual([{ pluginId: "example.plugin", version: "1.2.3", digestSha256: "b".repeat(64) }])
    } finally {
      root.unmount()
      desktopQueryClient.clear()
    }
  })
})
