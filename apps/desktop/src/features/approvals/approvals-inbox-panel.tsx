import { Fragment } from "react"

import { palette } from "../../app/theme.js"
import { CopyTextButton } from "../../ui/copy-text-button.js"
import { Pressable } from "../../ui/pressable.js"
import type { ApprovalRequest } from "@taugentic/desktop-protocol"
import type { ApprovalsInboxState } from "./use-approvals-inbox.js"

export function approvalTargetLabel(target: ApprovalRequest["target"]): string {
  switch (target.kind) {
    case "toolCall": return `toolCall · Tool: ${target.toolName}`
    case "fileWrite": return `fileWrite · Paths: ${target.paths.join(", ")}`
    case "processExec": return `processExec${target.command ? ` · Command: ${target.command}` : ""}`
    case "networkAccess": return `networkAccess${target.protocol ? ` · Protocol: ${target.protocol}` : ""}${target.host ? ` · Host: ${target.host}` : ""}`
    case "capsuleDispatch": return `capsuleDispatch${target.childRunId ? ` · Child run: ${target.childRunId}` : ""}${target.workspaceScope ? ` · Workspace scope: ${target.workspaceScope}` : ""}`
  }
}

export function ApprovalsInboxPanel(props: { inbox: ApprovalsInboxState; onOpenRun(runId: string): void; copyText?(text: string): void }) {
  const { inbox } = props
  return <div testId="approvals-inbox" style={{ display: "flex", flexDirection: "column", gap: 7, padding: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6 }}>
    <text style={{ color: palette.text, fontSize: 12, fontWeight: 600 }}>Approvals inbox</text>
    {inbox.loading && <text testId="approvals-loading" style={mutedStyle()}>Loading approvals…</text>}
    {inbox.error && <text testId="approvals-error" accessibilityRole="alert" accessibilityName={inbox.error} style={{ color: "#f08080", fontSize: 10 }}>{inbox.error}</text>}
    {!inbox.loading && !inbox.error && !inbox.approvals.length && <text testId="approvals-empty" style={mutedStyle()}>No pending approvals.</text>}
    {inbox.approvals.map((approval) => <Fragment key={approval.id}><div testId={`inbox-approval-${approval.id}`} style={{ display: "flex", flexDirection: "column", gap: 5, padding: 7, borderRadius: 4, backgroundColor: palette.panel }}>
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}><text style={{ color: palette.text, fontSize: 11 }}>{approval.reason}</text><CopyTextButton testId={`copy-inbox-approval-${approval.id}`} text={approval.reason} copyText={props.copyText} label="Copy reason" /></div>
      <text style={mutedStyle()}>Scope: {approval.scope} · Target: {approvalTargetLabel(approval.target)}</text>
      <text style={mutedStyle()}>Expires: {approval.expiresAtMs}</text>
      <text style={mutedStyle()}>Originating run: {approval.runId}</text>
      <div style={{ display: "flex", gap: 6 }}><Pressable testId={`open-approval-run-${approval.id}`} name={`Open originating run ${approval.runId}`} onPress={() => props.onOpenRun(approval.runId)} style={buttonStyle()}><text>Open run</text></Pressable><div style={{ flexGrow: 1 }} /><Pressable testId={`approve-inbox-${approval.id}`} name={`Approve approval ${approval.id}`} onPress={() => { void inbox.decide(approval.id, "approved") }} style={buttonStyle()}><text>Approve</text></Pressable><Pressable testId={`reject-inbox-${approval.id}`} name={`Reject approval ${approval.id}`} onPress={() => { void inbox.decide(approval.id, "rejected") }} style={buttonStyle()}><text>Reject</text></Pressable></div>
    </div></Fragment>)}
  </div>
}

function buttonStyle() { return { cursor: "pointer", padding: 6, backgroundColor: palette.accentDim, color: palette.textMuted, borderRadius: 4 } }
function mutedStyle() { return { color: palette.textMuted, fontSize: 10 } }
