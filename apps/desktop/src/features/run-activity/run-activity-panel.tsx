import { VirtualList } from "@regenrek/gpuix-react"
import { Fragment } from "react"

import type { ApprovalDecision, ApprovalId, ApprovalRequest, ArtifactId, PublicDaemonEvent, RunId } from "@taugentic/desktop-protocol"

import { palette } from "../../app/theme.js"
import { CopyTextButton } from "../../ui/copy-text-button.js"
import { Pressable } from "../../ui/pressable.js"
import { ApprovalsInboxPanel } from "../approvals/approvals-inbox-panel.js"
import type { ApprovalsInboxState } from "../approvals/use-approvals-inbox.js"
import type { ReturnTypeUseRunActivity } from "./types.js"

export function RunActivityPanel(props: { activity: ReturnTypeUseRunActivity; inbox?: ApprovalsInboxState; copyText?(text: string): void }) {
  const state = props.activity
  const olderActivityAvailable = !state.loadingOlderActivity
  const loadOlderActivity = () => { if (olderActivityAvailable) state.loadOlderActivity() }
  const olderRunsAvailable = !state.loadingOlderRuns
  const loadOlderRuns = () => { if (olderRunsAvailable) state.loadOlderRuns() }
  return <div testId="run-activity-panel" style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, gap: 10, padding: 12, overflow: "hidden" }}>
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}><text style={{ color: palette.text, fontSize: 15, fontWeight: 650 }}>Run & Activity</text><div style={{ flexGrow: 1 }} /><Pressable testId="refresh-run-activity" name="Refresh run activity" onPress={state.refresh} style={buttonStyle()}><text>Refresh</text></Pressable></div>
    {state.error && <text testId="run-activity-error" accessibilityRole="alert" accessibilityName={state.error} style={{ color: "#f08080" }}>{state.error}</text>}
    <div style={{ display: "flex", flexGrow: 1, minHeight: 0, gap: 10 }}>
    <div style={{ ...sectionStyle(), width: 250, minWidth: 180, minHeight: 0 }}><text style={headingStyle()}>Runs</text>
      {!state.runs.length && !state.loading && <text testId="run-empty" style={mutedStyle()}>No runs for this conversation.</text>}
      {!!state.runs.length && <div testId="run-history" style={{ display: "flex", flexDirection: "column", flexGrow: 1, minHeight: 0 }}><VirtualList itemCount={state.runs.length} estimatedItemHeight={36} renderItem={(index) => {
        const run = state.runs[index]
        if (!run) return null
        return <Fragment key={run.id}><Pressable testId={`run-${run.id}`} name={`Open run ${run.objectivePreview ?? run.id}`} selected={state.selectedRunId === run.id} onPress={() => state.selectRun(run.id)} style={{ ...rowStyle(), backgroundColor: state.selectedRunId === run.id ? palette.panelRaised : palette.panel }}><text style={{ color: palette.text, fontSize: 11 }}>{run.objectivePreview ?? run.id}</text><text style={mutedStyle()}>{run.status}</text></Pressable></Fragment>
      }} style={{ flexGrow: 1, minHeight: 0, width: "100%" }} /></div>}
      {state.hasOlderRuns && <Pressable testId="load-older-runs" name="Load older runs" disabled={!olderRunsAvailable} onPress={loadOlderRuns} style={buttonStyle(olderRunsAvailable)}><text>{olderRunsAvailable ? "Load older runs" : "Loading…"}</text></Pressable>}
    </div>
    <div testId="run-activity-content" style={{ display: "flex", flexDirection: "column", flexGrow: 1, minWidth: 0, minHeight: 0, gap: 10, overflow: "scroll" }}>
    {state.detail && <div testId="run-detail" style={sectionStyle()}><text style={headingStyle()}>Selected run</text><div style={{ display: "flex", alignItems: "center", gap: 6 }}><text style={{ color: palette.text, fontSize: 12 }}>{state.detail.summary.objective}</text><CopyTextButton testId="copy-run-objective" text={state.detail.summary.objective} copyText={props.copyText} label="Copy objective" /></div><text testId="run-detail-status" style={mutedStyle()}>{state.detail.summary.status}</text>{state.detail.recipeId && <text testId="run-recipe-provenance" style={mutedStyle()}>Recipe: {state.detail.recipeId}</text>}{state.detail.authProfileExhaustion && <text testId="run-auth-profile-exhaustion" style={{ color: "#f0b060" }}>{state.detail.authProfileExhaustion}</text>}{state.detail.contractViolation && <text testId="run-failure" style={{ color: "#f08080" }}>{state.detail.contractViolation.kind}</text>}</div>}
    {state.selectedRunId && cancellationEligible(state.detail?.summary.status) && <Pressable testId="cancel-selected-run" name="Cancel selected run" onPress={() => void state.cancel(state.selectedRunId as RunId)} style={buttonStyle()}><text>Cancel selected run</text></Pressable>}
    {state.switchEligible && <Pressable testId="switch-route-and-resume" name="Switch route and resume" onPress={() => void state.switchRouteAndResume()} style={buttonStyle()}><text>Switch route & resume</text></Pressable>}
    {props.inbox && <ApprovalsInboxPanel inbox={props.inbox} onOpenRun={(runId) => state.selectRun(runId as RunId)} copyText={props.copyText} />}
    <div style={sectionStyle()}><text style={headingStyle()}>Selected run approvals</text><Approvals approvals={state.approvals} onDecide={state.decide} copyText={props.copyText} /></div>
    <div style={sectionStyle()}><text style={headingStyle()}>Timeline</text><TimelineTree runs={state.timeline?.runs ?? []} />{state.timeline?.events.map((event) => { const artifactId = event.payload.kind === "artifact" ? event.payload.artifactId : undefined; const exhaustion = event.payload.kind === "run" ? event.payload.auth_profile_exhaustion : undefined; return <Fragment key={event.seq}><div testId={`timeline-${event.seq}`} style={rowStyle()}><text style={{ color: palette.text, fontSize: 11 }}>{event.label}</text><text style={mutedStyle()}>{event.status ?? event.kind}</text>{exhaustion && <text testId={`timeline-exhaustion-${event.seq}`} style={{ color: "#f0b060" }}>{exhaustion}</text>}{artifactId && <Pressable testId={`open-artifact-${artifactId}`} name="Open artifact" onPress={() => state.openArtifact(artifactId as ArtifactId)} style={buttonStyle()}><text>Artifact</text></Pressable>}</div></Fragment>})}</div>
    <div style={sectionStyle()}><text style={headingStyle()}>Replay</text>{state.replay.map((event) => <Fragment key={event.seq}><div testId={`replay-${event.seq}`} style={rowStyle()}><text style={mutedStyle()}>{event.seq}</text><text style={{ color: palette.text, fontSize: 11 }}>{replayLabel(event.event)}</text><text style={mutedStyle()}>{replayStatus(event.event)}</text></div></Fragment>)}</div>
    <div style={sectionStyle()}><text style={headingStyle()}>Activity</text>{state.hasOlderActivity && <Pressable testId="load-older-activity" name="Load older activity" disabled={!olderActivityAvailable} onPress={loadOlderActivity} style={buttonStyle(olderActivityAvailable)}><text>{olderActivityAvailable ? "Load older activity" : "Loading…"}</text></Pressable>}{state.activity.map((item) => <Fragment key={String(item.cursor.sequence)}><div testId={`activity-${item.cursor.sequence}`} style={rowStyle()}><text style={mutedStyle()}>{activityLabel(item.event)}</text></div></Fragment>)}</div>
    </div>
    </div>
  </div>
}

function TimelineTree(props: { runs: readonly { runId: string; depth: number; status: string }[] }) {
  return <>{props.runs.map((run) => <Fragment key={run.runId}><div testId={`timeline-run-${run.runId}`} style={{ ...rowStyle(), paddingLeft: 7 + run.depth * 14 }}><text style={{ color: palette.text, fontSize: 11 }}>{run.depth ? "↳ " : ""}{run.runId}</text><text style={mutedStyle()}>{run.status}</text></div></Fragment>)}</>
}

function Approvals(props: { approvals: readonly ApprovalRequest[]; onDecide(approvalId: ApprovalId, decision: ApprovalDecision): Promise<void>; copyText?(text: string): void }) {
  if (!props.approvals.length) return <text testId="approval-empty" style={mutedStyle()}>No pending approvals.</text>
  return <>{props.approvals.map((approval) => <Fragment key={approval.id}><div testId={`approval-${approval.id}`} style={rowStyle()}><text style={{ color: palette.text, fontSize: 11 }}>{approval.reason}</text><div style={{ display: "flex", gap: 6 }}><CopyTextButton testId={`copy-approval-${approval.id}`} text={approval.reason} copyText={props.copyText} label="Copy reason" /><Pressable testId={`approve-${approval.id}`} name="Approve" onPress={() => void props.onDecide(approval.id, "approved")} style={buttonStyle()}><text>Approve</text></Pressable><Pressable testId={`reject-${approval.id}`} name="Reject" onPress={() => void props.onDecide(approval.id, "rejected")} style={buttonStyle()}><text>Reject</text></Pressable></div></div></Fragment>)}</>
}

function activityLabel(event: PublicDaemonEvent): string {
  if ("run" in event && event.run.kind === "status" && event.run.payload.authProfileExhaustion) {
    return `Account ${event.run.payload.authProfileExhaustion}`
  }
  return Object.keys(event)[0] ?? "Activity"
}
function replayLabel(event: PublicDaemonEvent): string {
  if ("run" in event) return bounded(event.run.payload.reason ?? "Run status changed")
  if ("approval" in event) return `Approval ${event.approval.phase}`
  if ("artifact" in event) return bounded(`Artifact ${event.artifact.artifact.displayName}`)
  if ("contextReceipt" in event) return `Context receipt ${event.contextReceipt.phase}`
  if ("agentStream" in event) return event.agentStream.frame.kind === "toolCallStarted" ? bounded(`Tool ${event.agentStream.frame.toolName}`) : `Agent ${event.agentStream.frame.kind}`
  if ("tokenUsageRecorded" in event) return bounded(`Token usage ${event.tokenUsageRecorded.model}`)
  if ("budget" in event) return "Budget exceeded"
  if ("conflict" in event) return "Claim conflict warning"
  if ("runReconciledOnStartup" in event) return "Run reconciled on startup"
  return "Session status changed"
}
function replayStatus(event: PublicDaemonEvent): string {
  if ("run" in event) return event.run.payload.status
  if ("approval" in event) return event.approval.phase === "resolved" ? event.approval.resolution.decision : event.approval.phase
  if ("artifact" in event) return event.artifact.artifact.kind
  if ("contextReceipt" in event) return event.contextReceipt.phase
  if ("agentStream" in event) return event.agentStream.frame.kind === "toolCallCompleted" ? event.agentStream.frame.outcome : event.agentStream.frame.kind
  if ("budget" in event) return event.budget.phase
  if ("conflict" in event) return event.conflict.phase
  if ("runReconciledOnStartup" in event) return event.runReconciledOnStartup.prevStatus
  if ("tokenUsageRecorded" in event) return bounded(event.tokenUsageRecorded.provider)
  return event.session.status
}
function bounded(value: string) { return value.length > 120 ? `${value.slice(0, 117)}...` : value }
function cancellationEligible(status?: string): boolean { return status === "queued" || status === "running" || status === "waitingForApproval" }
function sectionStyle() { return { display: "flex" as const, flexDirection: "column" as const, gap: 6, padding: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6 } }
function rowStyle() { return { display: "flex" as const, alignItems: "center" as const, gap: 8, padding: 7, borderRadius: 4 } }
function buttonStyle(available = true) { return { cursor: available ? "pointer" : "default", padding: 6, backgroundColor: available ? palette.accentDim : palette.panelRaised, color: palette.textMuted, borderRadius: 4 } }
function headingStyle() { return { color: palette.text, fontSize: 12, fontWeight: 600 } }
function mutedStyle() { return { color: palette.textMuted, fontSize: 10 } }
