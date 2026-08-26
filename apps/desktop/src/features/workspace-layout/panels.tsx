import type { DockPanel } from "@gpuix/react"
import type { ApprovalDecision, ApprovalId, ApprovalRequest, RunStatus, SessionId } from "@taugentic/desktop-protocol"
import { Fragment } from "react"

import { palette } from "../../app/theme.js"

export type AssistantMessage = { id: string; text: string }

export type ConversationPanelProps = {
  title: string
  selectedConversationId?: SessionId
  messages: readonly AssistantMessage[]
  approvals: readonly ApprovalRequest[]
  objective: string
  error?: string
  canStart: boolean
  canCancel: boolean
  runStatus?: RunStatus
  onObjectiveChange(value: string): void
  onStart(): void
  onCancel(): void
  onDecideApproval(approvalId: ApprovalId, decision: ApprovalDecision): void
}

export function panelRegistry(props: ConversationPanelProps): readonly DockPanel[] {
  return [
    {
      id: "conversation",
      label: "Conversation",
      content: <ConversationPanel {...props} />,
      closable: false,
    },
    {
      id: "activity",
      label: "Activity",
      content: <ActivityPanel approvals={props.approvals} onDecide={props.onDecideApproval} />,
      closable: true,
    },
  ]
}

function ActivityPanel(props: { approvals: readonly ApprovalRequest[]; onDecide(approvalId: ApprovalId, decision: ApprovalDecision): void }) {
  return <div testId="activity-panel" style={{ display: "flex", flexDirection: "column", padding: 20, gap: 12, height: "100%", overflow: "scroll" }}>
    <text style={{ color: palette.text, fontSize: 15, fontWeight: 650 }}>Approvals</text>
    {!props.approvals.length && <text testId="approval-empty" style={{ color: palette.textMuted }}>No pending approvals.</text>}
    {props.approvals.map((approval) => <Fragment key={approval.id}><div testId={`approval-${approval.id}`} style={{ display: "flex", flexDirection: "column", gap: 8, padding: 12, borderWidth: 1, borderColor: palette.border, borderRadius: 6, backgroundColor: palette.panel }}>
      <text style={{ color: palette.text, fontSize: 13, fontWeight: 600 }}>{approval.reason}</text>
      <text style={{ color: palette.textMuted, fontSize: 11 }}>{approvalTargetLabel(approval.target)}</text>
      <div style={{ display: "flex", gap: 8 }}>
        <div testId={`approve-${approval.id}`} tabIndex={0} onClick={() => props.onDecide(approval.id, "approved")} style={{ cursor: "pointer", padding: 8, backgroundColor: palette.accentDim, color: palette.text }}><text>Approve</text></div>
        <div testId={`reject-${approval.id}`} tabIndex={0} onClick={() => props.onDecide(approval.id, "rejected")} style={{ cursor: "pointer", padding: 8, backgroundColor: palette.panelRaised, color: palette.text }}><text>Reject</text></div>
      </div>
    </div></Fragment>)}
  </div>
}

function approvalTargetLabel(target: ApprovalRequest["target"]): string {
  if (target.kind === "fileWrite") return target.paths.length ? `Write ${target.paths.join(", ")}` : "Write files"
  if (target.kind === "processExec") return target.command ? `Run ${target.command}` : "Run a command"
  if (target.kind === "networkAccess") {
    const endpoint = [target.protocol, target.host].filter(Boolean).join("://")
    return endpoint ? `Connect to ${endpoint}` : "Access the network"
  }
  if (target.kind === "capsuleDispatch") return "Delegate work to a child run"
  return `Use ${target.toolName}`
}

function ConversationPanel(props: ConversationPanelProps) {
  return <div testId="conversation-panel" style={{ display: "flex", flexDirection: "column", height: "100%", padding: 24, gap: 14, minWidth: 0 }}>
    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
      <text style={{ color: palette.text, fontSize: 21, fontWeight: 650 }}>{props.title}</text>
      {props.runStatus && <text testId="run-status" style={{ color: props.runStatus === "failed" || props.runStatus === "budgetExceeded" ? "#f08080" : props.runStatus === "running" ? palette.accent : palette.textMuted, fontSize: 11 }}>{runStatusLabel(props.runStatus)}</text>}
    </div>
    {props.error && <text testId="daemon-error" style={{ color: "#f08080", fontSize: 12 }}>{props.error}</text>}
    <div testId="conversation" style={{ display: "flex", flexDirection: "column", flexGrow: 1, gap: 8, overflow: "scroll" }}>
      {props.messages.map((message) => <Fragment key={message.id}><text testId={`assistant-message-${message.id}`} style={{ color: palette.textMuted, fontSize: 13 }}>{message.text}</text></Fragment>)}
      {props.selectedConversationId && !props.messages.length && <text testId="conversation-placeholder" style={{ color: palette.textMuted, fontSize: 13, userSelect: "text" }}>The assistant will stream its response here.</text>}
    </div>
    <input testId="run-objective" autoFocus value={props.objective} placeholder="Describe the work to run" onChange={(event) => props.onObjectiveChange(event.value ?? "")} onKeyDown={(event) => { if (event.key === "enter") props.onStart() }} style={{ height: 38, paddingLeft: 10, paddingRight: 10, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.panel }} />
    <div style={{ display: "flex", gap: 8 }}>
      <div testId="start-run" tabIndex={props.canStart ? 0 : -1} onClick={() => { if (props.canStart) props.onStart() }} style={{ padding: 8, backgroundColor: props.canStart ? palette.accentDim : palette.panelRaised, color: props.canStart ? palette.text : palette.textFaint, cursor: props.canStart ? "pointer" : "default" }}><text>Start run</text></div>
      <div testId="cancel-run" tabIndex={props.canCancel ? 0 : -1} onClick={() => { if (props.canCancel) props.onCancel() }} style={{ padding: 8, backgroundColor: props.canCancel ? palette.accentDim : palette.panelRaised, color: props.canCancel ? palette.text : palette.textFaint, cursor: props.canCancel ? "pointer" : "default" }}><text>Cancel run</text></div>
    </div>
  </div>
}

function runStatusLabel(status: RunStatus): string {
  if (status === "waitingForApproval") return "WAITING FOR APPROVAL"
  if (status === "budgetExceeded") return "BUDGET EXCEEDED"
  return status.toUpperCase()
}
