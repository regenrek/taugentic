import type { AgentRuntimeSelection, AgentRuntimeSnapshot } from "@taugentic/desktop-protocol"

import { palette } from "../../app/theme.js"

type RuntimeRoutePickerProps = {
  snapshot?: AgentRuntimeSnapshot
  draft?: Partial<AgentRuntimeSelection>
  pendingAuthMethodIds: readonly string[]
  onDraft(draft: Partial<AgentRuntimeSelection>): void
  onLogin(authMethodId: string): void
  onLogout(authProfileId: string): void
}

export function RuntimeRoutePicker(props: RuntimeRoutePickerProps) {
  const runtimeProfile = props.snapshot?.runtimeProfiles?.find((profile) => profile.id === props.draft?.runtimeProfileId)
  const provider = props.snapshot?.providers?.find((candidate) => candidate.id === runtimeProfile?.providerId)
  const connectedProfiles = props.snapshot?.authProfiles?.filter((profile) => (
    profile.profile.providerId === runtimeProfile?.providerId && profile.connectionState === "connected"
  )) ?? []

  return <div testId="runtime-route-picker" style={{ display: "flex", flexDirection: "column", gap: 7, padding: 10, backgroundColor: palette.panelRaised }}>
    <text style={{ color: palette.textMuted, fontSize: 11 }}>Run route</text>
    <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
      {props.snapshot?.runtimeProfiles?.map((profile) => <div testId={`runtime-profile-${profile.id}`} tabIndex={0} onClick={() => props.onDraft({ runtimeProfileId: profile.id })} style={{ cursor: "pointer", padding: 6, backgroundColor: props.draft?.runtimeProfileId === profile.id ? palette.accentDim : palette.panel }}><text>{profile.displayName}</text></div>)}
    </div>
    {runtimeProfile && <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
      {connectedProfiles.map((profile) => <div testId={`auth-profile-${profile.profile.id}`} tabIndex={0} onClick={() => props.onDraft({ authProfileId: profile.profile.id })} style={{ cursor: "pointer", padding: 6, backgroundColor: props.draft?.authProfileId === profile.profile.id ? palette.accentDim : palette.panel }}><text>{profile.profile.displayName}</text></div>)}
      {props.snapshot?.authMethods?.filter((method) => method.providerId === runtimeProfile.providerId).map((method) => <div testId={`login-auth-method-${method.id}`} tabIndex={0} onClick={() => props.onLogin(method.id)} style={{ cursor: "pointer", padding: 6, backgroundColor: palette.panel }}><text>{props.pendingAuthMethodIds.includes(method.id) ? `Connecting ${method.displayName} in browser...` : `Connect ${method.displayName}`}</text></div>)}
    </div>}
    {provider && <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
      {provider.models?.map((model) => <div testId={`runtime-model-${model.id}`} tabIndex={0} onClick={() => props.onDraft({ modelId: model.id })} style={{ cursor: "pointer", padding: 6, backgroundColor: props.draft?.modelId === model.id ? palette.accentDim : palette.panel }}><text>{model.displayName}</text></div>)}
    </div>}
    <div style={{ display: "flex", gap: 6 }}>
      {props.snapshot?.authProfiles?.filter((profile) => profile.canLogout).map((profile) => <div testId={`logout-auth-profile-${profile.profile.id}`} tabIndex={0} onClick={() => props.onLogout(profile.profile.id)} style={{ cursor: "pointer", padding: 6, backgroundColor: palette.panel }}><text>Disconnect {profile.profile.displayName}</text></div>)}
    </div>
  </div>
}
