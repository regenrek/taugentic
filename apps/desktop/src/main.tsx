import { render } from "@regenrek/gpuix-react"
import { QueryClientProvider } from "@tanstack/react-query"

import { App } from "./app/App.js"
import { desktopRuntime, startWorkspaceShell } from "./features/runtime/workspace-shell.js"
import { desktopQueryClient } from "./platform/daemon/query-client.js"
import { desktopSettings } from "./platform/settings/desktop-settings.js"
import { nativeDesktopSettingsPersistence } from "./platform/settings/native-desktop-settings.js"

async function bootstrap(): Promise<void> {
  await desktopSettings.initialize(nativeDesktopSettingsPersistence(desktopRuntime.bridge))
  await startWorkspaceShell()
  render(<QueryClientProvider client={desktopQueryClient}><App /></QueryClientProvider>, {
    title: "Taugentic",
    width: 1440,
    height: 900,
    titlebarTransparent: true,
    windowBackground: "blurred",
    trafficLightX: 16,
    trafficLightY: 18,
  })
}

void bootstrap()
