import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import type { DaemonDiagnostics } from "@taugentic/desktop-protocol"

import { DiagnosticsPanel } from "../src/features/diagnostics/diagnostics-panel.js"
import { diagnosticsQuery, diagnosticsQueryKey } from "../src/platform/daemon/diagnostics-query.js"
import type { DesktopRuntime } from "../src/platform/daemon/desktop-runtime.js"

const diagnostics: DaemonDiagnostics = {
  uptimeMs: "1234",
  inFlightRpcCount: 2,
  inFlightCapsuleRunCount: 1,
  recentErrorCount: 3,
  recentErrors: [{ occurredAtMs: "1", source: "private.method", message: "private diagnostic failure" }],
  tokenUsage: { totalTokens: "24", promptTokens: "12", completionTokens: "10", cachedTokens: "2", reasoningTokens: "4" },
  worktreeCount: 5,
  claimCount: 6,
  sandbox: { os: "private-os", sandboxKind: "private-sandbox", helperAvailable: true, restrictedTokenJob: false, appcontainer: false, filesystemAllowlist: true, networkDefaultDeny: true, networkDestinationAllowlist: false },
  providerHealth: [{ providerId: "private-provider-id", displayName: "Provider One", status: "ready", message: "private provider failure" }],
}

function click(renderer: ReturnType<typeof createTestRoot>["renderer"], testId: string) {
  const element = renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id) ?? []
  renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

describe("M8 system diagnostics", () => {
  it("keeps one disabled-until-open query lifecycle with no automatic refetches", async () => {
    let calls = 0
    const runtime = { diagnosticsSnapshot: async () => { calls += 1; return diagnostics } } as DesktopRuntime
    const query = diagnosticsQuery(runtime)

    expect(diagnosticsQueryKey).toEqual(["daemon", "diagnostics"])
    expect(query.retry).toBe(false)
    expect(query.refetchInterval).toBe(false)
    expect(query.refetchOnReconnect).toBe(false)
    expect(query.refetchOnWindowFocus).toBe(false)
    expect(calls).toBe(0)
    expect(await runtime.diagnosticsSnapshot()).toEqual(diagnostics)
    expect(calls).toBe(1)
  })

  it("renders only safe generated snapshot fields across panel states and closes by mouse, Enter, and Space", () => {
    const { render, renderer, unmount } = createTestRoot()
    const closes: string[] = []
    try {
      render(<DiagnosticsPanel state="loading" onClose={() => closes.push("loading")} />)
      expect(renderer.getAutomationTree()).toContain("Loading diagnostics")
      render(<DiagnosticsPanel state="unavailable" onClose={() => closes.push("unavailable")} />)
      expect(renderer.getAutomationTree()).toContain("Diagnostics are unavailable")
      render(<DiagnosticsPanel state="error" onClose={() => closes.push("error")} />)
      expect(renderer.getAutomationTree()).toContain("Diagnostics could not be loaded")
      render(<DiagnosticsPanel state="ready" diagnostics={diagnostics} onClose={() => closes.push("ready")} />)
      const tree = renderer.getAutomationTree()
      expect(tree).not.toContain("Loading diagnostics")
      expect(tree).not.toContain("Diagnostics are unavailable")
      expect(tree).not.toContain("Diagnostics could not be loaded")
      expect(tree).toContain("Provider One")
      expect(tree).toContain("ready")
      expect(tree).not.toContain("private diagnostic failure")
      expect(tree).not.toContain("private provider failure")
      expect(tree).not.toContain("private-provider-id")
      expect(tree).not.toContain("private-os")
      expect(tree).not.toContain("private-sandbox")
      const close = renderer.findByTestId("close-system-diagnostics")!
      click(renderer, "close-system-diagnostics")
      renderer.nativeSimulateKeystrokes(close.id, "enter")
      renderer.nativeSimulateKeystrokes(close.id, "space")
      expect(closes).toEqual(["ready", "ready", "ready"])
    } finally { unmount() }
  })
})
