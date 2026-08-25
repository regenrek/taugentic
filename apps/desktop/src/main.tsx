import { render } from "@gpuix/react"

import { App } from "./app/App.js"

render(<App />, {
  title: "Taugentic",
  width: 1440,
  height: 900,
  titlebarTransparent: true,
  windowBackground: "blurred",
  trafficLightX: 16,
  trafficLightY: 18,
})
