import type { AgentRuntimeSelection, AgentRuntimeSnapshot, AuthProfilePreferences } from "@taugentic/desktop-protocol"
import { Fragment, useState, type ReactNode } from "react"

import { fontSize, palette } from "../../app/theme.js"

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

type ActionProps = {
  testId: string
  name: string
  selected?: boolean
  expanded?: boolean
  role?: "button" | "radio"
  onActivate(): void
  children: ReactNode
}

function Action({ testId, name, selected, expanded, role = "button", onActivate, children }: ActionProps) {
  return <div testId={testId} tabIndex={0} accessibilityRole={role} accessibilityName={name} accessibilityChecked={role === "radio" ? selected : undefined} accessibilityExpanded={expanded} onClick={onActivate} onKeyDown={(event) => { if (activates(event)) onActivate() }} style={{ cursor: "pointer", padding: 7, borderRadius: 5, backgroundColor: selected ? palette.accentDim : palette.panel }}>
    <text>{children}</text>
  </div>
}

function RouteSection({ title, detail, children }: { title: string; detail: string; children: ReactNode }) {
  return <div style={{ display: "flex", flexDirection: "column", gap: 6, padding: 8, borderRadius: 6, backgroundColor: palette.panel }}>
    <text style={{ color: palette.textMuted, fontSize: fontSize(10), fontWeight: 700 }}>{title}</text>
    <text style={{ color: palette.textFaint, fontSize: fontSize(11) }}>{detail}</text>
    {children}
  </div>
}

export function RuntimeRoutePicker(props: RuntimeRoutePickerProps) {
  const [expanded, setExpanded] = useState(false)
  const [labels, setLabels] = useState<Record<string, string>>({})
  const runtimeProfile = props.snapshot?.runtimeProfiles?.find((profile) => profile.id === props.draft?.runtimeProfileId)
  const provider = props.snapshot?.providers?.find((candidate) => candidate.id === runtimeProfile?.providerId)
  const selectedProfile = props.snapshot?.authProfiles?.find((profile) => profile.profile.id === props.draft?.authProfileId)
  const model = provider?.models?.find((candidate) => candidate.id === props.draft?.modelId)
  const authMethod = props.snapshot?.authMethods?.find((method) => method.id === runtimeProfile?.authMethodId)
  const accountProfiles = runtimeProfile?.authMethodId
    ? (props.snapshot?.authProfiles?.filter((profile) => profile.profile.providerId === runtimeProfile.providerId && profile.profile.authMethodId === runtimeProfile.authMethodId) ?? []).slice().sort((left, right) => left.preferences.order - right.preferences.order)
    : []
  const connectedProfiles = accountProfiles.filter((profile) => profile.connectionState === "connected")
  const routeSummary = [runtimeProfile?.displayName, selectedProfile?.profile.displayName, model?.displayName].filter(Boolean).join(" · ")
  const readiness = routeReadiness(props.snapshot, runtimeProfile, selectedProfile, model)
  const modelCapability = provider?.modelCapability

  return <div testId="runtime-route-picker" style={{ display: "flex", flexDirection: "column", gap: expanded ? 9 : 0, padding: 12, backgroundColor: palette.panelRaised, borderBottomWidth: 1, borderColor: palette.border }}>
    <div style={{ display: "flex", alignItems: "center", gap: 10, minHeight: 28 }}>
      <text style={{ color: palette.textFaint, fontSize: fontSize(10), fontWeight: 700 }}>RUN ROUTE</text>
      <text testId="runtime-route-summary" style={{ color: routeSummary ? palette.textMuted : palette.warning, fontSize: fontSize(11) }}>{routeSummary || readiness}</text>
      <div style={{ flexGrow: 1 }} />
      <Action testId="runtime-route-toggle" name="Change run route" expanded={expanded} onActivate={() => setExpanded((value) => !value)}>{expanded ? "Done" : "Change"}</Action>
    </div>
    {expanded && <div testId="runtime-route-options" accessibilityRole="group" accessibilityName="Run route editor" style={{ display: "flex", flexDirection: "column", gap: 8, paddingTop: 4 }}>
      <text testId="runtime-route-readiness" accessibilityRole="status" accessibilityName={`Run route: ${readiness}`} style={{ color: readiness === "Ready to run." ? palette.accent : palette.warning, fontSize: fontSize(11) }}>{readiness}</text>
      <RouteSection title="1. RUNTIME PROFILE" detail={runtimeProfile ? `Selected: ${runtimeProfile.displayName}${provider?.health?.message ? ` · ${provider.health.message}` : ""}` : "Choose the runtime profile for this run."}>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {props.snapshot?.runtimeProfiles?.map((profile) => <Action key={profile.id} testId={`runtime-profile-${profile.id}`} role="radio" name={`Use runtime profile ${profile.displayName}`} selected={props.draft?.runtimeProfileId === profile.id} onActivate={() => props.onDraft({ runtimeProfileId: profile.id })}>{profile.displayName}</Action>)}
        </div>
      </RouteSection>
      <RouteSection title="2. CONNECTED ACCOUNT" detail={accountDetail(runtimeProfile?.authMethodId, accountProfiles.length, connectedProfiles.length)}>
        {runtimeProfile?.authMethodId && <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          {accountProfiles.map((profile, index) => <Fragment key={profile.profile.id}><div style={{ display: "flex", flexDirection: "column", gap: 5, padding: 6, borderRadius: 5, backgroundColor: palette.panelRaised }}>
            {profile.connectionState === "connected"
              ? <Action testId={`auth-profile-${profile.profile.id}`} role="radio" name={`Use account ${profile.preferences.label}`} selected={props.draft?.authProfileId === profile.profile.id} onActivate={() => props.onDraft({ authProfileId: profile.profile.id })}>{accountSummary(profile)}</Action>
              : <text testId={`auth-profile-unavailable-${profile.profile.id}`} accessibilityRole="status" accessibilityName={`${profile.preferences.label} unavailable: ${unavailableAccountCause(profile)}`} style={{ color: palette.warning, fontSize: fontSize(11) }}>{profile.preferences.label} · unavailable · {unavailableAccountCause(profile)}</text>}
            <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
              <input testId={`auth-profile-label-${profile.profile.id}`} accessibilityName={`Account label for ${profile.preferences.label}`} value={labels[profile.profile.id] ?? profile.preferences.label} onChange={(event) => setLabels((current) => ({ ...current, [profile.profile.id]: event.value ?? "" }))} />
              <Action testId={`save-auth-profile-${profile.profile.id}`} name={`Save ${profile.preferences.label}`} onActivate={() => props.onPreferences(profile.profile.id, { ...profile.preferences, label: labels[profile.profile.id] ?? profile.preferences.label })}>Save account</Action>
              <Action testId={`default-auth-profile-${profile.profile.id}`} name={`Make ${profile.preferences.label} default`} onActivate={() => props.onPreferences(profile.profile.id, { ...profile.preferences, isDefault: true })}>Make default</Action>
              {index > 0 && <Action testId={`move-auth-profile-up-${profile.profile.id}`} name={`Move ${profile.preferences.label} up`} onActivate={() => props.onPreferences(profile.profile.id, { ...profile.preferences, order: profile.preferences.order - 1 })}>Move up</Action>}
              {index < accountProfiles.length - 1 && <Action testId={`move-auth-profile-down-${profile.profile.id}`} name={`Move ${profile.preferences.label} down`} onActivate={() => props.onPreferences(profile.profile.id, { ...profile.preferences, order: profile.preferences.order + 1 })}>Move down</Action>}
              {profile.canLogout && <Action testId={`logout-auth-profile-${profile.profile.id}`} name={`Disconnect ${profile.preferences.label}`} onActivate={() => props.onLogout(profile.profile.id)}>Disconnect</Action>}
            </div>
          </div></Fragment>)}
          {authMethod && <Action testId={`login-auth-method-${authMethod.id}`} name={`Connect ${authMethod.displayName}`} onActivate={() => props.onLogin(authMethod.id)}>{props.pendingAuthMethodIds.includes(authMethod.id) ? `Connecting ${authMethod.displayName} in browser...` : `Connect ${authMethod.displayName}`}</Action>}
        </div>}
      </RouteSection>
      <RouteSection title="3. MODEL" detail={modelDetail(modelCapability)}>
        {modelCapability?.canSetModel && <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          {provider?.models?.map((candidate) => <Action key={candidate.id} testId={`runtime-model-${candidate.id}`} role="radio" name={`Use model ${candidate.displayName}`} selected={props.draft?.modelId === candidate.id} onActivate={() => props.onDraft({ modelId: candidate.id })}>{candidate.displayName}</Action>)}
        </div>}
      </RouteSection>
    </div>}
  </div>
}

function routeReadiness(snapshot: AgentRuntimeSnapshot | undefined, runtime: NonNullable<AgentRuntimeSnapshot["runtimeProfiles"]>[number] | undefined, account: NonNullable<AgentRuntimeSnapshot["authProfiles"]>[number] | undefined, model: NonNullable<NonNullable<AgentRuntimeSnapshot["providers"]>[number]["models"]>[number] | undefined): string {
  if (!snapshot) return "Route details are unavailable."
  if (!runtime) return "Choose a runtime profile."
  if (!account) return "Choose a connected account."
  if (account.connectionState !== "connected") return `Selected account is unavailable: ${unavailableAccountCause(account)}.`
  if (account.exhaustion) return `Selected account is unavailable: ${account.exhaustion}.`
  if (!model) return "Choose a model."
  return "Ready to run."
}

function accountDetail(authMethodId: string | null | undefined, count: number, connectedCount: number): string {
  if (!authMethodId) return "This runtime does not expose an account method."
  if (!count) return "No accounts are available for this runtime. Connect an account to continue."
  return connectedCount ? "Choose a connected account. Account actions affect only the daemon profile." : "No connected accounts are available. Connect an account to continue."
}

function modelDetail(capability: { availability: string; canSetModel: boolean; detail?: string | null } | undefined): string {
  if (!capability) return "Choose a runtime profile to see its model availability."
  if (capability.canSetModel) return "Choose a model explicitly for this run."
  return capability.detail ?? `Models are unavailable (${capability.availability}).`
}

function accountSummary(profile: { preferences: AuthProfilePreferences; profile: { planTier?: string | null }; usage: { kind: "unavailable" } | { kind: "observed"; windows: Array<{ label: string; remaining?: string | null; limit?: string | null }> }; exhaustion: string | null }): string {
  return `${profile.preferences.label}${profile.preferences.isDefault ? " · default" : ""} · connected · ${profile.profile.planTier ?? "plan unavailable"} · ${usageSummary(profile.usage)}${profile.exhaustion ? ` · ${profile.exhaustion}` : ""}`
}

function unavailableAccountCause(profile: { connectionState: string; exhaustion: string | null; lastError?: string | null }): string {
  return profile.lastError ?? profile.exhaustion ?? profile.connectionState
}

function usageSummary(usage: { kind: "unavailable" } | { kind: "observed"; windows: Array<{ label: string; remaining?: string | null; limit?: string | null }> }): string {
  if (usage.kind === "unavailable") return "usage unavailable"
  return usage.windows.length ? usage.windows.map((window) => `${window.label}: ${window.remaining ?? "?"}/${window.limit ?? "?"}`).join(", ") : "usage observed"
}
