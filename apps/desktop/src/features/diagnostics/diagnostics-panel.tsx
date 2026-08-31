import type { DaemonDiagnostics } from "@taugentic/desktop-protocol"

import { fontSize, palette } from "../../app/theme.js"

type DiagnosticsPanelState = "loading" | "unavailable" | "error" | "ready"

export function DiagnosticsPanel(props: {
  state: DiagnosticsPanelState
  diagnostics?: DaemonDiagnostics
  onClose(): void
}) {
  const diagnostics = props.diagnostics
  return <div testId="system-diagnostics-panel" accessibilityRole="dialog" accessibilityName="System Diagnostics" style={panelStyle()}>
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <text style={{ color: palette.text, fontSize: fontSize(15), fontWeight: 700 }}>System Diagnostics</text><div style={{ flexGrow: 1 }} />
      <div testId="close-system-diagnostics" tabIndex={0} accessibilityRole="button" accessibilityName="Close System Diagnostics" onClick={props.onClose} onKeyDown={(event) => { if (activates(event)) props.onClose() }} style={buttonStyle()}><text>Close</text></div>
    </div>
    {props.state === "loading" && <text testId="system-diagnostics-loading" style={mutedStyle()}>Loading diagnostics…</text>}
    {props.state === "unavailable" && <text testId="system-diagnostics-unavailable" accessibilityRole="alert" style={mutedStyle()}>Diagnostics are unavailable while the daemon is disconnected.</text>}
    {props.state === "error" && <text testId="system-diagnostics-error" accessibilityRole="alert" style={mutedStyle()}>Diagnostics could not be loaded.</text>}
    {props.state === "ready" && diagnostics && <DiagnosticsFacts diagnostics={diagnostics} />}
  </div>
}

function DiagnosticsFacts({ diagnostics }: { diagnostics: DaemonDiagnostics }) {
  return <div testId="system-diagnostics-facts" style={{ display: "flex", flexDirection: "column", gap: 8 }}>
    <Fact label="Uptime" value={diagnostics.uptimeMs} />
    <Fact label="In-flight RPCs" value={String(diagnostics.inFlightRpcCount)} />
    <Fact label="In-flight runs" value={String(diagnostics.inFlightCapsuleRunCount)} />
    <Fact label="Recent errors" value={String(diagnostics.recentErrorCount)} />
    <Fact label="Worktrees" value={String(diagnostics.worktreeCount)} />
    <Fact label="Claims" value={String(diagnostics.claimCount)} />
    <Fact label="Total tokens" value={diagnostics.tokenUsage.totalTokens ?? "Unavailable"} />
    <Fact label="Prompt tokens" value={diagnostics.tokenUsage.promptTokens ?? "Unavailable"} />
    <Fact label="Completion tokens" value={diagnostics.tokenUsage.completionTokens ?? "Unavailable"} />
    <Fact label="Cached tokens" value={diagnostics.tokenUsage.cachedTokens ?? "Unavailable"} />
    <Fact label="Reasoning tokens" value={diagnostics.tokenUsage.reasoningTokens ?? "Unavailable"} />
    <text style={headingStyle()}>Sandbox capabilities</text>
    <BooleanFact label="Helper available" value={diagnostics.sandbox.helperAvailable} />
    <BooleanFact label="Restricted token jobs" value={diagnostics.sandbox.restrictedTokenJob} />
    <BooleanFact label="AppContainer" value={diagnostics.sandbox.appcontainer} />
    <BooleanFact label="Filesystem allowlist" value={diagnostics.sandbox.filesystemAllowlist} />
    <BooleanFact label="Network default deny" value={diagnostics.sandbox.networkDefaultDeny} />
    <BooleanFact label="Network destination allowlist" value={diagnostics.sandbox.networkDestinationAllowlist} />
    <text style={headingStyle()}>Providers</text>
    {!diagnostics.providerHealth.length && <text testId="system-diagnostics-no-providers" style={mutedStyle()}>No provider status is available.</text>}
    {diagnostics.providerHealth.map((provider) => <ProviderFact key={provider.providerId} provider={provider} />)}
  </div>
}

function ProviderFact({ provider }: { provider: DaemonDiagnostics["providerHealth"][number] }) { return <div testId="system-diagnostics-provider" style={rowStyle()}><text style={{ color: palette.text, fontSize: fontSize(11) }}>{provider.displayName}</text><div style={{ flexGrow: 1 }} /><text style={mutedStyle()}>{provider.status}</text></div> }
function Fact({ label, value }: { label: string; value: string }) { return <div style={rowStyle()}><text style={mutedStyle()}>{label}</text><div style={{ flexGrow: 1 }} /><text style={{ color: palette.text, fontSize: fontSize(11) }}>{value}</text></div> }
function BooleanFact({ label, value }: { label: string; value: boolean }) { return <Fact label={label} value={value ? "Available" : "Unavailable"} /> }
function activates(event: { key?: string }): boolean { return event.key === "enter" || event.key === "space" }
function panelStyle() { return { position: "absolute" as const, right: 18, top: 50, width: 420, maxHeight: 740, overflow: "scroll" as const, display: "flex" as const, flexDirection: "column" as const, gap: 10, padding: 12, backgroundColor: palette.panel, borderWidth: 1, borderColor: palette.border, borderRadius: 8 } }
function rowStyle() { return { display: "flex" as const, alignItems: "center" as const, gap: 8, padding: 7, backgroundColor: palette.canvas, borderRadius: 4 } }
function buttonStyle() { return { cursor: "pointer", padding: 7, backgroundColor: palette.panelRaised } }
function headingStyle() { return { color: palette.textFaint, fontSize: fontSize(10), fontWeight: 700 } }
function mutedStyle() { return { color: palette.textMuted, fontSize: fontSize(11) } }
