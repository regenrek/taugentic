import { render } from "@gpuix/react"
import { QueryClientProvider } from "@tanstack/react-query"

import { App } from "./app/App.js"
import { startWorkspaceShell } from "./features/runtime/workspace-shell-machine.js"
import { navigationQueryClient } from "./platform/daemon/navigation-query.js"

void startWorkspaceShell()

render(<QueryClientProvider client={navigationQueryClient}><App /></QueryClientProvider>, {
  title: "Taugentic",
  width: 1440,
  height: 900,
  titlebarTransparent: true,
  windowBackground: "blurred",
  trafficLightX: 16,
  trafficLightY: 18,
})
