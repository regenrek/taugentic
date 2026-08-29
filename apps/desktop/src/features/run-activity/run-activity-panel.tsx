import { Fragment } from "react"

import type { ApprovalDecision, ApprovalId, ApprovalRequest, ArtifactId, PublicDaemonEvent, RunId } from "@taugentic/desktop-protocol"

import { palette } from "../../app/theme.js"
import { ApprovalsInboxPanel } from "../approvals/approvals-inbox-panel.js"
import type { ApprovalsInboxState } from "../approvals/use-approvals-inbox.js"
import type { ReturnTypeUseRunActivity } from "./types.js"

function activates(event: { key?: string }): boolean { return event.key === "enter" || event.key === "space" }

export function RunActivityPanel(props: { activity: ReturnTypeUseRunActivity; inbox?: ApprovalsInboxState }) {
  const state = props.activity
  const olderActivityAvailable = !state.loadingOlderActivity
  const loadOlderActivity = () => { if (olderActivityAvailable) state.loadOlderActivity() }
  return <div testId="run-activity-panel" style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, gap: 10, padding: 12, overflow: "scroll" }}>
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}><text style={{ color: palette.text, fontSize: 15, fontWeight: 650 }}>Run & Activity</text><div style={{ flexGrow: 1 }} /><div testId="refresh-run-activity" tabIndex={0} accessibilityRole="button" accessibilityName="Refresh run activity" accessibilityDisabled={false} onClick={state.refresh} onKeyDown={(event) => { if (activates(event)) state.refresh() }} style={buttonStyle()}><text>Refresh</text></div></div>
    {state.error && <text testId="run-activity-error" accessibilityRole="alert" accessibilityName={state.error} style={{ color: "#f08080" }}>{state.error}</text>}
    <div style={sectionStyle()}><text style={headingStyle()}>Runs</text>
      {!state.runs.length && !state.loading && <text testId="run-empty" style={mutedStyle()}>No runs for this conversation.</text>}
      {state.runs.map((run) => <Fragment key={run.id}><div testId={`run-${run.id}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Open run ${run.objectivePreview ?? run.id}`} accessibilitySelected={state.selectedRunId === run.id} onClick={() => state.selectRun(run.id)} onKeyDown={(event) => { if (activates(event)) state.selectRun(run.id) }} style={{ ...rowStyle(), backgroundColor: state.selectedRunId === run.id ? palette.panelRaised : palette.panel }}><text style={{ color: palette.text, fontSize: 11 }}>{run.relationship.kind === "root" ? "" : "↳ "}{run.objectivePreview ?? run.id}</text><text style={mutedStyle()}>{run.status}</text></div></Fragment>)}
    </div>
    {state.detail && <div testId="run-detail" style={sectionStyle()}><text style={headingStyle()}>Selected run</text><text style={{ color: palette.text, fontSize: 12 }}>{state.detail.summary.objective}</text><text testId="run-detail-status" style={mutedStyle()}>{state.detail.summary.status}</text>{state.detail.recipeId && <text testId="run-recipe-provenance" style={mutedStyle()}>Recipe: {state.detail.recipeId}</text>}{state.detail.authProfileExhaustion && <text testId="run-auth-profile-exhaustion" style={{ color: "#f0b060" }}>{state.detail.authProfileExhaustion}</text>}{state.detail.contractViolation && <text testId="run-failure" style={{ color: "#f08080" }}>{state.detail.contractViolation.kind}</text>}</div>}
    {state.selectedRunId && cancellationEligible(state.detail?.summary.status) && <div testId="cancel-selected-run" tabIndex={0} accessibilityRole="button" accessibilityName="Cancel selected run" accessibilityDisabled={false} onClick={() => void state.cancel(state.selectedRunId as RunId)} onKeyDown={(event) => { if (activates(event)) void state.cancel(state.selectedRunId as RunId) }} style={buttonStyle()}><text>Cancel selected run</text></div>}
    {state.switchEligible && <div testId="switch-account-and-resume" tabIndex={0} accessibilityRole="button" accessibilityName="Switch account and resume" accessibilityDisabled={false} onClick={() => void state.switchAccountAndResume()} onKeyDown={(event) => { if (activates(event)) void state.switchAccountAndResume() }} style={buttonStyle()}><text>Switch account & resume</text></div>}
    {props.inbox && <ApprovalsInboxPanel inbox={props.inbox} onOpenRun={(runId) => state.selectRun(runId as RunId)} />}
    <div style={sectionStyle()}><text style={headingStyle()}>Selected run approvals</text><Approvals approvals={state.approvals} onDecide={state.decide} /></div>
    <div style={sectionStyle()}><text style={headingStyle()}>Timeline</text>{state.timeline?.events.map((event) => { const artifactId = event.payload.kind === "artifact" ? event.payload.artifactId : undefined; const exhaustion = event.payload.kind === "run" ? event.payload.auth_profile_exhaustion : undefined; return <Fragment key={event.seq}><div testId={`timeline-${event.seq}`} style={rowStyle()}><text style={{ color: palette.text, fontSize: 11 }}>{event.label}</text><text style={mutedStyle()}>{event.status ?? event.kind}</text>{exhaustion && <text testId={`timeline-exhaustion-${event.seq}`} style={{ color: "#f0b060" }}>{exhaustion}</text>}{artifactId && <div testId={`open-artifact-${artifactId}`} tabIndex={0} accessibilityRole="button" accessibilityName="Open artifact" accessibilityDisabled={false} onClick={() => state.openArtifact(artifactId as ArtifactId)} onKeyDown={(keyEvent) => { if (activates(keyEvent)) state.openArtifact(artifactId as ArtifactId) }} style={buttonStyle()}><text>Artifact</text></div>}</div></Fragment>})}</div>
    <div style={sectionStyle()}><text style={headingStyle()}>Replay</text>{state.replay.map((event) => <Fragment key={event.seq}><div testId={`replay-${event.seq}`} style={rowStyle()}><text style={mutedStyle()}>{event.seq}</text></div></Fragment>)}</div>
    <div style={sectionStyle()}><text style={headingStyle()}>Activity</text>{state.hasOlderActivity && <div testId="load-older-activity" tabIndex={olderActivityAvailable ? 0 : -1} accessibilityRole="button" accessibilityName="Load older activity" accessibilityDisabled={!olderActivityAvailable} onClick={loadOlderActivity} onKeyDown={(event) => { if (activates(event)) loadOlderActivity() }} style={buttonStyle(olderActivityAvailable)}><text>{olderActivityAvailable ? "Load older activity" : "Loading…"}</text></div>}{state.activity.map((item) => <Fragment key={String(item.cursor.sequence)}><div testId={`activity-${item.cursor.sequence}`} style={rowStyle()}><text style={mutedStyle()}>{activityLabel(item.event)}</text></div></Fragment>)}</div>
  </div>
}

function Approvals(props: { approvals: readonly ApprovalRequest[]; onDecide(approvalId: ApprovalId, decision: ApprovalDecision): Promise<void> }) {
  if (!props.approvals.length) return <text testId="approval-empty" style={mutedStyle()}>No pending approvals.</text>
  return <>{props.approvals.map((approval) => <Fragment key={approval.id}><div testId={`approval-${approval.id}`} style={rowStyle()}><text style={{ color: palette.text, fontSize: 11 }}>{approval.reason}</text><div style={{ display: "flex", gap: 6 }}><div testId={`approve-${approval.id}`} tabIndex={0} accessibilityRole="button" accessibilityName="Approve" accessibilityDisabled={false} onClick={() => void props.onDecide(approval.id, "approved")} onKeyDown={(event) => { if (activates(event)) void props.onDecide(approval.id, "approved") }} style={buttonStyle()}><text>Approve</text></div><div testId={`reject-${approval.id}`} tabIndex={0} accessibilityRole="button" accessibilityName="Reject" accessibilityDisabled={false} onClick={() => void props.onDecide(approval.id, "rejected")} onKeyDown={(event) => { if (activates(event)) void props.onDecide(approval.id, "rejected") }} style={buttonStyle()}><text>Reject</text></div></div></div></Fragment>)}</>
}

function activityLabel(event: PublicDaemonEvent): string {
  if ("run" in event && event.run.kind === "status" && event.run.payload.authProfileExhaustion) {
    return `Account ${event.run.payload.authProfileExhaustion}`
  }
  return Object.keys(event)[0] ?? "Activity"
}
function cancellationEligible(status?: string): boolean { return status === "queued" || status === "running" || status === "waitingForApproval" }
function sectionStyle() { return { display: "flex" as const, flexDirection: "column" as const, gap: 6, padding: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6 } }
function rowStyle() { return { display: "flex" as const, alignItems: "center" as const, gap: 8, padding: 7, borderRadius: 4 } }
function buttonStyle(available = true) { return { cursor: available ? "pointer" : "default", padding: 6, backgroundColor: available ? palette.accentDim : palette.panelRaised, color: palette.textMuted, borderRadius: 4 } }
function headingStyle() { return { color: palette.text, fontSize: 12, fontWeight: 600 } }
function mutedStyle() { return { color: palette.textMuted, fontSize: 10 } }
