import { render } from "@regenrek/gpuix-react"
import { QueryClientProvider } from "@tanstack/react-query"
import { useSyncExternalStore } from "react"

import { App } from "./app/App.js"
import { DesktopStartupPresentation } from "./desktop-startup.js"
import { desktopRuntime, startWorkspaceShell } from "./features/runtime/workspace-shell.js"
import { desktopQueryClient } from "./platform/daemon/query-client.js"
import { desktopSettings } from "./platform/settings/desktop-settings.js"
import { nativeDesktopSettingsPersistence } from "./platform/settings/native-desktop-settings.js"

const desktopStartup = new DesktopStartupPresentation()

function DesktopPresentation() {
  const startupError = useSyncExternalStore(desktopStartup.subscribe, () => desktopStartup.error())
  return <>
    {startupError && <div testId="desktop-startup-error" accessibilityRole="alert" accessibilityName={startupError}><text>{startupError}</text></div>}
    <App />
  </>
}

desktopStartup.start({
  renderPrimaryWindow() {
    render(<QueryClientProvider client={desktopQueryClient}><DesktopPresentation /></QueryClientProvider>, {
    title: "Taugentic",
    width: 1440,
    height: 900,
    titlebarTransparent: true,
    windowBackground: "blurred",
    trafficLightX: 16,
    trafficLightY: 18,
    })
  },
  async bootstrapWorkspace() {
    await desktopSettings.initialize(nativeDesktopSettingsPersistence(desktopRuntime.bridge))
    await startWorkspaceShell()
  },
})
