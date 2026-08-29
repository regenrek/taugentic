import type { RunStatus, VoiceEvent, VoicePermissionState } from "@taugentic/desktop-protocol"

import { palette } from "../../app/theme.js"

export type VoicePanelProps = {
  visible: boolean
  permission: VoicePermissionState
  state?: VoiceEvent
  runStatus?: RunStatus
  onRequestPermission(): void
}

function activates(event: { key?: string }): boolean {
  return event.key === "enter" || event.key === "space"
}

function phaseLabel(props: VoicePanelProps): string {
  if (props.runStatus === "completed") return "Voice session completed"
  if (props.runStatus === "failed" || props.runStatus === "budgetExceeded") return "Voice session failed"
  if (props.runStatus === "cancelled") return "Voice session cancelled"
  if (props.state?.phase === "connecting") return "Connecting voice"
  if (props.state?.phase === "speaking") return "Assistant speaking"
  if (props.state?.phase === "listening") return "Listening"
  return "Voice ready"
}

export function VoicePanel(props: VoicePanelProps) {
  if (!props.visible) return null
  const needsPermission = props.permission !== "authorized"
  const canRequestPermission = props.permission === "notDetermined"
  return <div testId="voice-panel" accessibilityRole="group" accessibilityName="Voice session" style={{ display: "flex", alignItems: "center", gap: 10, padding: 10, borderWidth: 1, borderColor: palette.border, borderRadius: 8, backgroundColor: palette.panelRaised }}>
    <text testId="voice-status" style={{ color: props.runStatus === "failed" || props.runStatus === "budgetExceeded" ? "#f08080" : palette.text, fontSize: 12 }}>{needsPermission ? (props.permission === "denied" || props.permission === "restricted" ? "Microphone access is unavailable" : "Microphone access is required") : phaseLabel(props)}</text>
    {canRequestPermission && <div testId="request-voice-permission" tabIndex={0} accessibilityRole="button" accessibilityName="Allow microphone access" onClick={props.onRequestPermission} onKeyDown={(event) => { if (activates(event)) props.onRequestPermission() }} style={{ padding: 7, backgroundColor: palette.accentDim, cursor: "pointer" }}><text style={{ color: palette.text, fontSize: 11 }}>Allow microphone</text></div>}
  </div>
}
