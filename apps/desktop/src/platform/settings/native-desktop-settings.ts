import type { NativeDaemonBridge } from "@taugentic/desktop-daemon-native"

import type { DesktopSettingsPersistence } from "./desktop-settings.js"

/** The only desktop adapter for native opaque-document persistence. */
export function nativeDesktopSettingsPersistence(bridge: NativeDaemonBridge): DesktopSettingsPersistence {
  return {
    read: () => bridge.readDesktopSettings(),
    write: async (documentJson) => { await bridge.writeDesktopSettings(documentJson) },
  }
}
