import type { VoiceEvent, VoicePermissionState } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"

/** The desktop's typed, non-audio view of the native voice boundary. */
export function readVoicePermission(runtime: DesktopRuntime): VoicePermissionState {
  return runtime.voicePermissionState()
}

export function observeVoice(
  runtime: DesktopRuntime,
  onPermission: (permission: VoicePermissionState) => void,
  onState: (event: VoiceEvent) => void,
): void {
  onPermission(readVoicePermission(runtime))
  runtime.subscribeVoiceState(onState)
}

export function requestVoicePermission(
  runtime: DesktopRuntime,
  onPermission: (permission: VoicePermissionState) => void,
): void {
  runtime.requestVoicePermission(onPermission)
}
