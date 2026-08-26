import { NativeDaemonBridge } from "@taugentic/desktop-daemon-native"

import type { DesktopDaemonLifecycleProjection } from "@taugentic/desktop-protocol"

import { decodeProtocolJson } from "./protocol-json.js"

export type DesktopRuntime = {
  start(): Promise<void>
  close(): Promise<void>
  bridge: NativeDaemonBridge
  subscribeLifecycle(listener: (projection: DesktopDaemonLifecycleProjection) => void): Promise<DesktopDaemonLifecycleProjection>
}

/** The sole desktop owner of the redacted Rust bridge instance. */
export function createDesktopRuntime(bridge: NativeDaemonBridge = new NativeDaemonBridge()): DesktopRuntime {

  return {
    bridge,
    async start() {
      await bridge.start()
    },
    async close() {
      await bridge.close()
    },
    async subscribeLifecycle(listener) {
      const bufferedProjections: DesktopDaemonLifecycleProjection[] = []
      let initialDelivered = false
      const initialProjectionJson = await bridge.subscribeLifecycle((projectionJson) => {
        const projection = decodeProtocolJson<DesktopDaemonLifecycleProjection>(projectionJson)
        if (!initialDelivered) {
          bufferedProjections.push(projection)
          return
        }
        listener(projection)
      })
      const initialProjection = decodeProtocolJson<DesktopDaemonLifecycleProjection>(initialProjectionJson)
      listener(initialProjection)
      for (const projection of bufferedProjections) listener(projection)
      initialDelivered = true
      return initialProjection
    },
  }
}
