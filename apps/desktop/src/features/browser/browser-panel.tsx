import type { WorkbenchBrowser } from "./use-workbench-browser.js"
import { palette } from "../../app/theme.js"

export function BrowserPanel({ browser, visible, onClose }: { browser: WorkbenchBrowser; visible: boolean; onClose(): void }) {
  return <div testId="browser-panel" style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, gap: 7, padding: 8 }}>
    <div style={{ display: "flex", gap: 6, paddingRight: 34, position: "relative" }}>
      {browser.profileId && <>
        <div testId="browser-back" onClick={() => browser.history("back")} style={{ padding: 6, backgroundColor: palette.panelRaised, opacity: browser.canGoBack ? 1 : .5 }}><text>Back</text></div>
        <div testId="browser-forward" onClick={() => browser.history("forward")} style={{ padding: 6, backgroundColor: palette.panelRaised, opacity: browser.canGoForward ? 1 : .5 }}><text>Forward</text></div>
        <div testId="browser-reload" onClick={() => browser.history("reload")} style={{ padding: 6, backgroundColor: palette.panelRaised }}><text>Reload</text></div>
        <div testId="browser-clear-data" onClick={() => browser.clearData()} style={{ padding: 6, backgroundColor: palette.panelRaised }}><text>Clear data</text></div>
      </>}
      <div testId="browser-close" accessibilityRole="button" accessibilityName="Close browser" onClick={onClose} style={{ position: "absolute", right: 0, top: 0, padding: 6, backgroundColor: palette.panelRaised, cursor: "pointer" }}><text>×</text></div>
    </div>
    {browser.profileId ? <>
      <input testId="browser-url" value={browser.url} onChange={(event) => browser.navigate(event.value ?? "")} style={{ padding: 7, backgroundColor: palette.canvas, color: palette.text, borderWidth: 1, borderColor: palette.border }} />
      {browser.denial && <text testId="browser-denial" accessibilityRole="alert" style={{ color: "#f08080" }}>{browser.denial}</text>}
      <browser-surface profileId={browser.profileId} visible={visible} navigationIntent={browser.navigationIntent} actionDecision={browser.decision} clearDataRequestId={browser.clearDataRequestId} onBrowserNavigation={browser.navigation} onBrowserLoading={browser.loadingState} onBrowserActionRequested={browser.action} style={{ flexGrow: 1, minHeight: 0, borderWidth: 1, borderColor: palette.border }} />
    </> : <text>Preparing browser…</text>}
  </div>
}
