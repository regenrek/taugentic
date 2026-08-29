import type { AgentRuntimeSelection, AgentRuntimeSnapshot, AuthProfilePreferences } from "@taugentic/desktop-protocol"
import { Fragment, useState } from "react"

import { palette } from "../../app/theme.js"

function activates(event: { key?: string }): boolean { return event.key === "enter" || event.key === "space" }

type RuntimeRoutePickerProps = {
  snapshot?: AgentRuntimeSnapshot
  draft?: Partial<AgentRuntimeSelection>
  pendingAuthMethodIds: readonly string[]
  onDraft(draft: Partial<AgentRuntimeSelection>): void
  onLogin(authMethodId: string): void
  onLogout(authProfileId: string): void
  onPreferences(authProfileId: string, preferences: AuthProfilePreferences): void
}

export function RuntimeRoutePicker(props: RuntimeRoutePickerProps) {
  const [expanded, setExpanded] = useState(false)
  const [labels, setLabels] = useState<Record<string, string>>({})
  const runtimeProfile = props.snapshot?.runtimeProfiles?.find((profile) => profile.id === props.draft?.runtimeProfileId)
  const provider = props.snapshot?.providers?.find((candidate) => candidate.id === runtimeProfile?.providerId)
  const authProfile = props.snapshot?.authProfiles?.find((profile) => profile.profile.id === props.draft?.authProfileId)
  const model = provider?.models?.find((candidate) => candidate.id === props.draft?.modelId)
  const providerProfiles = runtimeProfile?.authMethodId
    ? (props.snapshot?.authProfiles?.filter((profile) => (
      profile.profile.providerId === runtimeProfile.providerId
      && profile.profile.authMethodId === runtimeProfile.authMethodId
    )) ?? []).slice().sort((left, right) => left.preferences.order - right.preferences.order)
    : []
  const connectedProfiles = providerProfiles.filter((profile) => profile.connectionState === "connected")
  const routeSummary = [runtimeProfile?.displayName, authProfile?.profile.displayName, model?.displayName].filter(Boolean).join(" · ")

  return <div testId="runtime-route-picker" style={{ display: "flex", flexDirection: "column", gap: expanded ? 9 : 0, paddingLeft: 12, paddingRight: 12, paddingTop: 8, paddingBottom: 8, backgroundColor: palette.panelRaised, borderBottomWidth: 1, borderColor: palette.border }}>
    <div style={{ display: "flex", alignItems: "center", gap: 10, minHeight: 28 }}>
      <text style={{ color: palette.textFaint, fontSize: 10, fontWeight: 700 }}>RUN ROUTE</text>
      <text testId="runtime-route-summary" style={{ color: routeSummary ? palette.textMuted : palette.warning, fontSize: 11 }}>{routeSummary || "Choose route, account, and model"}</text>
      <div style={{ flexGrow: 1 }} />
      <div testId="runtime-route-toggle" tabIndex={0} accessibilityRole="button" accessibilityName="Change run route" accessibilityExpanded={expanded} onClick={() => setExpanded((value) => !value)} onKeyDown={(event) => { if (activates(event)) setExpanded((value) => !value) }} style={{ cursor: "pointer", padding: 6, borderRadius: 6, backgroundColor: palette.panel }}><text style={{ color: palette.textMuted, fontSize: 11 }}>{expanded ? "Done" : "Change"}</text></div>
    </div>
    {expanded && <div testId="runtime-route-options" style={{ display: "flex", flexDirection: "column", gap: 8, paddingTop: 4 }}>
      <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
        {props.snapshot?.runtimeProfiles?.map((profile) => <Fragment key={profile.id}><div testId={`runtime-profile-${profile.id}`} tabIndex={0} accessibilityRole="radio" accessibilityName={profile.displayName} accessibilityChecked={props.draft?.runtimeProfileId === profile.id} onClick={() => props.onDraft({ runtimeProfileId: profile.id })} onKeyDown={(event) => { if (activates(event)) props.onDraft({ runtimeProfileId: profile.id }) }} style={{ cursor: "pointer", padding: 6, borderRadius: 5, backgroundColor: props.draft?.runtimeProfileId === profile.id ? palette.accentDim : palette.panel }}><text>{profile.displayName}</text></div></Fragment>)}
      </div>
      {runtimeProfile && <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
        {connectedProfiles.map((profile, profileIndex) => <Fragment key={profile.profile.id}><div testId={`auth-profile-${profile.profile.id}`} tabIndex={0} accessibilityRole="radio" accessibilityName={`Use ${profile.preferences.label}`} accessibilityChecked={props.draft?.authProfileId === profile.profile.id} onClick={() => props.onDraft({ authProfileId: profile.profile.id })} onKeyDown={(event) => { if (activates(event)) props.onDraft({ authProfileId: profile.profile.id }) }} style={{ cursor: "pointer", padding: 6, borderRadius: 5, backgroundColor: props.draft?.authProfileId === profile.profile.id ? palette.accentDim : palette.panel }}><text>{profile.preferences.label}{profile.preferences.isDefault ? " · default" : ""} · connected · {profile.profile.planTier ?? "plan unavailable"} · {usageSummary(profile.usage)}{profile.exhaustion ? ` · ${profile.exhaustion}` : ""} · order {profile.preferences.order + 1}</text></div><input testId={`auth-profile-label-${profile.profile.id}`} accessibilityName={`Account label for ${profile.preferences.label}`} value={labels[profile.profile.id] ?? profile.preferences.label} onChange={(event) => setLabels((current) => ({ ...current, [profile.profile.id]: event.value ?? "" }))} /><div testId={`save-auth-profile-${profile.profile.id}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Save ${profile.preferences.label}`} onClick={() => props.onPreferences(profile.profile.id, { ...profile.preferences, label: labels[profile.profile.id] ?? profile.preferences.label })} onKeyDown={(event) => { if (activates(event)) props.onPreferences(profile.profile.id, { ...profile.preferences, label: labels[profile.profile.id] ?? profile.preferences.label }) }} style={{ cursor: "pointer", padding: 6, borderRadius: 5, backgroundColor: palette.panel }}><text>Save account</text></div><div testId={`default-auth-profile-${profile.profile.id}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Make ${profile.preferences.label} default`} onClick={() => props.onPreferences(profile.profile.id, { ...profile.preferences, isDefault: true })} onKeyDown={(event) => { if (activates(event)) props.onPreferences(profile.profile.id, { ...profile.preferences, isDefault: true }) }} style={{ cursor: "pointer", padding: 6, borderRadius: 5, backgroundColor: palette.panel }}><text>Make default</text></div>{profileIndex > 0 && <div testId={`move-auth-profile-up-${profile.profile.id}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Move ${profile.preferences.label} up`} onClick={() => props.onPreferences(profile.profile.id, { ...profile.preferences, order: profile.preferences.order - 1 })} onKeyDown={(event) => { if (activates(event)) props.onPreferences(profile.profile.id, { ...profile.preferences, order: profile.preferences.order - 1 }) }} style={{ cursor: "pointer", padding: 6, borderRadius: 5, backgroundColor: palette.panel }}><text>Move up</text></div>}{profileIndex < connectedProfiles.length - 1 && <div testId={`move-auth-profile-down-${profile.profile.id}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Move ${profile.preferences.label} down`} onClick={() => props.onPreferences(profile.profile.id, { ...profile.preferences, order: profile.preferences.order + 1 })} onKeyDown={(event) => { if (activates(event)) props.onPreferences(profile.profile.id, { ...profile.preferences, order: profile.preferences.order + 1 }) }} style={{ cursor: "pointer", padding: 6, borderRadius: 5, backgroundColor: palette.panel }}><text>Move down</text></div>}</Fragment>)}
        {props.snapshot?.authMethods?.filter((method) => method.id === runtimeProfile.authMethodId).map((method) => <Fragment key={method.id}><div testId={`login-auth-method-${method.id}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Connect ${method.displayName}`} onClick={() => props.onLogin(method.id)} onKeyDown={(event) => { if (activates(event)) props.onLogin(method.id) }} style={{ cursor: "pointer", padding: 6, borderRadius: 5, backgroundColor: palette.panel }}><text>{props.pendingAuthMethodIds.includes(method.id) ? `Connecting ${method.displayName} in browser...` : `Connect ${method.displayName}`}</text></div></Fragment>)}
      </div>}
      {provider && <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
        {provider.models?.map((candidate) => <Fragment key={candidate.id}><div testId={`runtime-model-${candidate.id}`} tabIndex={0} accessibilityRole="radio" accessibilityName={candidate.displayName} accessibilityChecked={props.draft?.modelId === candidate.id} onClick={() => { props.onDraft({ modelId: candidate.id }); setExpanded(false) }} onKeyDown={(event) => { if (activates(event)) { props.onDraft({ modelId: candidate.id }); setExpanded(false) } }} style={{ cursor: "pointer", padding: 6, borderRadius: 5, backgroundColor: props.draft?.modelId === candidate.id ? palette.accentDim : palette.panel }}><text>{candidate.displayName}</text></div></Fragment>)}
      </div>}
      <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
        {providerProfiles.filter((profile) => profile.canLogout).map((profile) => <Fragment key={profile.profile.id}><div testId={`logout-auth-profile-${profile.profile.id}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Disconnect ${profile.preferences.label}`} onClick={() => props.onLogout(profile.profile.id)} onKeyDown={(event) => { if (activates(event)) props.onLogout(profile.profile.id) }} style={{ cursor: "pointer", padding: 6, borderRadius: 5, backgroundColor: palette.panel }}><text>Disconnect {profile.preferences.label}</text></div></Fragment>)}
      </div>
    </div>}
  </div>
}

function usageSummary(usage: { kind: "unavailable" } | { kind: "observed", windows: Array<{ label: string, remaining?: string | null, limit?: string | null }> }): string {
  if (usage.kind === "unavailable") return "usage unavailable"
  return usage.windows.length
    ? usage.windows.map((window) => `${window.label}: ${window.remaining ?? "?"}/${window.limit ?? "?"}`).join(", ")
    : "usage observed"
}
