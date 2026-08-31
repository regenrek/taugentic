import { useEffect, useState } from "react"
import type { BrowserActionDecision, BrowserActionRequestedEvent, BrowserLoadingEvent } from "@regenrek/gpuix-react"
import type { BrowserActionRequest, BrowserActionResult, BrowserNavigationKind } from "@taugentic/desktop-protocol"
import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"

export type WorkbenchBrowserRuntime = Pick<DesktopRuntime, "browserProfile" | "browserAction" | "clearBrowserData">

export type WorkbenchBrowser = {
  profileId?: string
  url: string
  loading: boolean
  canGoBack: boolean
  canGoForward: boolean
  denial?: string
  navigationIntent?: { requestId: string; kind: BrowserNavigationKind; url?: string }
  decision?: BrowserActionDecision
  clearDataRequestId?: string
  navigate(url: string): void
  history(kind: Exclude<BrowserNavigationKind, "navigate">): void
  clearData(): void
  navigation(event: { browserUrl: string }): void
  loadingState(event: BrowserLoadingEvent): void
  action(event: BrowserActionRequestedEvent): void
}

export function useWorkbenchBrowser(runtime: WorkbenchBrowserRuntime, enabled: boolean): WorkbenchBrowser {
  const [profileId, setProfileId] = useState<string>()
  const [url, setUrl] = useState("https://example.com")
  const [loading, setLoading] = useState(false)
  const [canGoBack, setCanGoBack] = useState(false)
  const [canGoForward, setCanGoForward] = useState(false)
  const [denial, setDenial] = useState<string>()
  const [navigationIntent, setNavigationIntent] = useState<WorkbenchBrowser["navigationIntent"]>()
  const [decision, setDecision] = useState<BrowserActionDecision>()
  const [clearDataRequestId, setClearDataRequestId] = useState<string>()
  useEffect(() => { if (!enabled) return; void runtime.browserProfile().then((result) => setProfileId(result.profile.id)).catch(() => setDenial("Browser profile is unavailable.")) }, [enabled, runtime])
  const decide = (result: BrowserActionResult) => { setDecision({ requestId: result.requestId, decision: result.decision }); setDenial(result.reason ?? undefined) }
  const request = (kind: BrowserNavigationKind, requestedUrl?: string) => {
    const requestId = crypto.randomUUID()
    setNavigationIntent({ requestId, kind, url: requestedUrl })
  }
  return { profileId, url, loading, canGoBack, canGoForward, denial, navigationIntent, decision, clearDataRequestId,
    navigate: (next) => { setUrl(next); request("navigate", next) }, history: (kind) => request(kind),
    clearData: () => {
      if (!profileId) return
      const requestId = crypto.randomUUID()
      void runtime.clearBrowserData({ requestId, profileId }).then((result) => {
        decide(result)
        if (result.requestId === requestId && result.decision === "allow") setClearDataRequestId(requestId)
      }).catch(() => setDenial("Browser data could not be cleared."))
    },
    navigation: (event) => { setUrl(event.browserUrl) }, loadingState: (event) => { setLoading(event.browserIsLoading); setCanGoBack(event.browserCanGoBack); setCanGoForward(event.browserCanGoForward) },
    action: (event) => {
      const navigation = (() => {
        switch (event.browserActionKind) {
          case "navigationIntent":
            return event.browserNavigationIntent === undefined ? undefined : {
              kind: event.browserNavigationIntent,
              url: event.browserUrl,
            }
          case "navigationAction":
          case "navigationResponse":
            return { kind: "navigate" as const, url: event.browserUrl }
          case "downloadDestination":
            return undefined
        }
      })()
      const action: BrowserActionRequest = {
        requestId: event.browserRequestId,
        profileId: event.browserProfileId,
        kind: event.browserActionKind,
        navigation,
        shouldPerformDownload: event.browserActionKind === "navigationAction" ? event.browserShouldPerformDownload : undefined,
        canShowMimeType: event.browserActionKind === "navigationResponse" ? event.browserCanShowMimeType : undefined,
      }
      void runtime.browserAction(action).then(decide).catch(() => decide({
        requestId: event.browserRequestId,
        decision: "cancel",
        reason: "Browser action could not be authorized.",
      }))
    },
  }
}
