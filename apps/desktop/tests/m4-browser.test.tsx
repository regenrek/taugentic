import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"
import React, { act } from "react"

import { BrowserPanel } from "../src/features/browser/browser-panel.js"
import { useWorkbenchBrowser, type WorkbenchBrowserRuntime } from "../src/features/browser/use-workbench-browser.js"
import type { BrowserActionRequest } from "@taugentic/desktop-protocol"
import type { BrowserActionRequestedEvent } from "@regenrek/gpuix-react"

async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0))
}

function createBrowserRuntime(overrides: Partial<WorkbenchBrowserRuntime> = {}): WorkbenchBrowserRuntime {
  return {
    async browserProfile() { return { profile: { id: "profile" } } },
    async browserAction(request) { return { requestId: request.requestId, decision: "cancel" } },
    async clearBrowserData(request) { return { requestId: request.requestId, decision: "cancel" } },
    ...overrides,
  }
}

function nativeBrowserAction(event: Omit<BrowserActionRequestedEvent, "elementId" | "eventType">): BrowserActionRequestedEvent {
  return {
    elementId: 1,
    eventType: "browserActionRequested",
    ...event,
  }
}

describe("M4 Browser", () => {
  it("renders one BrowserSurface with presentation-only history and one close action", () => {
    const root = createTestRoot()
    let closed = false
    try {
      root.render(<BrowserPanel visible browser={{
        profileId: "profile",
        url: "https://example.com",
        loading: false,
        canGoBack: false,
        canGoForward: false,
        denial: "Downloads are not available yet.",
        navigationIntent: { requestId: "navigate", kind: "navigate", url: "https://example.com" },
        decision: { requestId: "navigate", decision: "allow" },
        navigate() {}, history() {}, clearData() {}, navigation() {}, loadingState() {}, action() {},
      }} onClose={() => { closed = true }} />)
      expect(root.renderer.findByTestId("browser-panel")).toBeDefined()
      expect(root.renderer.findByTestId("browser-url")).toBeDefined()
      expect(root.renderer.findByTestId("browser-denial")).toBeDefined()
      expect(root.renderer.getAutomationTree()).toContain("Downloads are not available yet.")
      const close = root.renderer.findByTestId("browser-close")
      const bounds = close ? root.renderer.getElementBounds(close.id) : null
      if (!bounds) throw new Error("Browser close control was not painted.")
      const [x = 0, y = 0, width = 0, height = 0] = bounds
      root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
      expect(closed).toBe(true)
    } finally {
      root.unmount()
    }
  })

  it("emits native clear data only after the daemon allows the exact request", async () => {
    const clearRequests: Array<{ requestId: string; profileId: string }> = []
    const clearResponses = [Promise.withResolvers<{ requestId: string; decision: "allow" }>(), Promise.withResolvers<{ requestId: string; decision: "allow" }>()]
    const runtime = createBrowserRuntime({
      clearBrowserData(request: { requestId: string; profileId: string }) {
        clearRequests.push(request)
        return clearResponses[clearRequests.length - 1]!.promise
      },
    })
    let browser!: ReturnType<typeof useWorkbenchBrowser>
    function Harness() {
      browser = useWorkbenchBrowser(runtime, true)
      return <BrowserPanel visible browser={browser} onClose={() => {}} />
    }
    const root = createTestRoot()
    try {
      await act(async () => { root.render(<Harness />) })
      root.renderer.flush()
      await act(async () => { browser.clearData() })
      root.renderer.flush()
      expect(clearRequests).toHaveLength(1)

      await act(async () => {
        clearResponses[0]!.resolve({ requestId: `${clearRequests[0]!.requestId}-wrong`, decision: "allow" })
        await clearResponses[0]!.promise
      })
      root.renderer.flush()
      expect(browser.clearDataRequestId).toBeUndefined()

      await act(async () => { browser.clearData() })
      root.renderer.flush()
      expect(clearRequests).toHaveLength(2)

      await act(async () => {
        clearResponses[1]!.resolve({ requestId: clearRequests[1]!.requestId, decision: "allow" })
        await clearResponses[1]!.promise
      })
      root.renderer.flush()
      expect(browser.clearDataRequestId).toBe(clearRequests[1]?.requestId)
    } finally {
      root.unmount()
    }
  })

  it("forwards a stale native profile action and applies the daemon cancellation", async () => {
    const actions: BrowserActionRequest[] = []
    const runtime = createBrowserRuntime({
      async browserProfile() { return { profile: { id: "current-profile" } } },
      async browserAction(action) {
        actions.push(action)
        return { requestId: "native-action", decision: "cancel" as const, reason: "Browser action is not authorized." }
      },
    })
    let browser!: ReturnType<typeof useWorkbenchBrowser>
    function Harness() {
      browser = useWorkbenchBrowser(runtime, true)
      return <BrowserPanel visible browser={browser} onClose={() => {}} />
    }
    const root = createTestRoot()
    try {
      root.render(<Harness />)
      await settle()
      browser.action(nativeBrowserAction({
        browserRequestId: "native-action",
        browserActionKind: "navigationAction",
        browserProfileId: "stale-profile",
        browserUrl: "https://example.com",
      }))
      await settle()
      expect(actions).toEqual([{
        requestId: "native-action",
        profileId: "stale-profile",
        kind: "navigationAction",
        navigation: { kind: "navigate", url: "https://example.com" },
        shouldPerformDownload: undefined,
        canShowMimeType: undefined,
      }])
      expect(browser.decision).toEqual({ requestId: "native-action", decision: "cancel" })
      expect(browser.denial).toBe("Browser action is not authorized.")
    } finally {
      root.unmount()
    }
  })

  it("forwards native download facts only for their matching action variants", async () => {
    const actions: BrowserActionRequest[] = []
    const runtime = createBrowserRuntime({
      async browserAction(action) {
        actions.push(action)
        return { requestId: action.requestId, decision: "download" as const }
      },
    })
    let browser!: ReturnType<typeof useWorkbenchBrowser>
    function Harness() {
      browser = useWorkbenchBrowser(runtime, true)
      return <BrowserPanel visible browser={browser} onClose={() => {}} />
    }
    const root = createTestRoot()
    try {
      root.render(<Harness />)
      await settle()
      browser.action(nativeBrowserAction({ browserRequestId: "action", browserActionKind: "navigationAction", browserProfileId: "profile", browserUrl: "https://example.com/file", browserShouldPerformDownload: true, browserCanShowMimeType: false }))
      browser.action(nativeBrowserAction({ browserRequestId: "response", browserActionKind: "navigationResponse", browserProfileId: "profile", browserUrl: "https://example.com/file", browserShouldPerformDownload: true, browserCanShowMimeType: false }))
      await settle()
      expect(actions).toEqual([
        { requestId: "action", profileId: "profile", kind: "navigationAction", navigation: { kind: "navigate", url: "https://example.com/file" }, shouldPerformDownload: true, canShowMimeType: undefined },
        { requestId: "response", profileId: "profile", kind: "navigationResponse", navigation: { kind: "navigate", url: "https://example.com/file" }, shouldPerformDownload: undefined, canShowMimeType: false },
      ])
      expect(browser.decision).toEqual({ requestId: "response", decision: "download" })
    } finally {
      root.unmount()
    }
  })

  it("forwards a malformed navigation intent without a navigation fact so the daemon cancels it", async () => {
    const actions: BrowserActionRequest[] = []
    const runtime = createBrowserRuntime({
      async browserAction(action) {
        actions.push(action)
        return { requestId: action.requestId, decision: "cancel" as const, reason: "This browser action is not available." }
      },
    })
    let browser!: ReturnType<typeof useWorkbenchBrowser>
    function Harness() {
      browser = useWorkbenchBrowser(runtime, true)
      return <BrowserPanel visible browser={browser} onClose={() => {}} />
    }
    const root = createTestRoot()
    try {
      root.render(<Harness />)
      await settle()
      browser.action(nativeBrowserAction({ browserRequestId: "malformed", browserActionKind: "navigationIntent", browserProfileId: "profile", browserUrl: "https://example.com" }))
      await settle()
      expect(actions).toEqual([{
        requestId: "malformed",
        profileId: "profile",
        kind: "navigationIntent",
        navigation: undefined,
        shouldPerformDownload: undefined,
        canShowMimeType: undefined,
      }])
      expect(browser.decision).toEqual({ requestId: "malformed", decision: "cancel" })
    } finally {
      root.unmount()
    }
  })

  it("cancels the exact pending native request when browserAction RPC rejects", async () => {
    const runtime = createBrowserRuntime({
      async browserAction() { throw new Error("transport failure") },
    })
    let browser!: ReturnType<typeof useWorkbenchBrowser>
    function Harness() {
      browser = useWorkbenchBrowser(runtime, true)
      return <BrowserPanel visible browser={browser} onClose={() => {}} />
    }
    const root = createTestRoot()
    try {
      root.render(<Harness />)
      await settle()
      browser.action(nativeBrowserAction({ browserRequestId: "rejected-request", browserActionKind: "navigationAction", browserProfileId: "profile", browserUrl: "https://example.com", browserShouldPerformDownload: false }))
      await settle()
      expect(browser.decision).toEqual({ requestId: "rejected-request", decision: "cancel" })
      expect(browser.denial).toBe("Browser action could not be authorized.")
    } finally {
      root.unmount()
    }
  })
})
