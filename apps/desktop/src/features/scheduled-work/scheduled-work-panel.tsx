import { Fragment } from "react"

import { palette } from "../../app/theme.js"
import type { ScheduledWorkState } from "./use-scheduled-work.js"

function activates(event: { key?: string }): boolean { return event.key === "enter" || event.key === "space" }

function runIdFor(state: import("@taugentic/desktop-protocol").ScheduledWorkOccurrence["state"]): string | undefined {
  return "run_id" in state && state.run_id ? state.run_id : undefined
}

function isCancellable(state: import("@taugentic/desktop-protocol").ScheduledWorkOccurrence["state"]): boolean {
  return state.kind === "pending" || state.kind === "preparing" || state.kind === "preparationCancellationRequested" || state.kind === "claimed"
}

function actionStyle(enabled: boolean) {
  return { cursor: enabled ? "pointer" : "default", padding: 6, backgroundColor: enabled ? palette.panelRaised : palette.panel, opacity: enabled ? 1 : 0.6 }
}

/** Accessible sidebar surface for creating and observing daemon-owned one-shot Scheduled Work. */
export function ScheduledWorkPanel({ scheduledWork, onOpenRun }: { scheduledWork: ScheduledWorkState; onOpenRun(runId: string): void }) {
  return <div testId="scheduled-work" style={{ display: "flex", flexDirection: "column", gap: 7, paddingTop: 8 }}>
    <text style={{ color: palette.textFaint, fontSize: 10, fontWeight: 700 }}>SCHEDULED WORK</text>
    <input testId="scheduled-work-objective" accessibilityName="Work objective" value={scheduledWork.objective} placeholder="Work objective" onChange={(event) => scheduledWork.setObjective(event.value ?? "")} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
    <input testId="scheduled-work-due-at-ms" accessibilityName="Due time in Unix milliseconds" value={scheduledWork.dueAtMs} placeholder="Due time (Unix ms)" onChange={(event) => scheduledWork.setDueAtMs(event.value ?? "")} onSubmit={() => scheduledWork.create()} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
    <div testId="create-scheduled-work" tabIndex={scheduledWork.canCreate ? 0 : -1} accessibilityRole="button" accessibilityName="Schedule work" accessibilityDisabled={!scheduledWork.canCreate} onClick={() => scheduledWork.create()} onKeyDown={(event) => { if (activates(event)) scheduledWork.create() }} style={actionStyle(scheduledWork.canCreate)}><text>Schedule work</text></div>
    {scheduledWork.error && <text testId="scheduled-work-error" accessibilityRole="alert" style={{ color: "#f08080", fontSize: 11 }}>{scheduledWork.error}</text>}
    {scheduledWork.mutationError && <text testId="scheduled-work-mutation-error" accessibilityRole="alert" style={{ color: "#f08080", fontSize: 11 }}>{scheduledWork.mutationError}</text>}
    {scheduledWork.loading && !scheduledWork.occurrences.length && <text testId="scheduled-work-loading" style={{ color: palette.textMuted, fontSize: 11 }}>Loading scheduled work…</text>}
    {!scheduledWork.loading && !scheduledWork.occurrences.length && <text testId="scheduled-work-empty" style={{ color: palette.textMuted, fontSize: 11 }}>No scheduled work.</text>}
    {scheduledWork.occurrences.map((occurrence) => {
      const runId = runIdFor(occurrence.state)
      const cancelEnabled = !scheduledWork.busy && isCancellable(occurrence.state)
      return <Fragment key={occurrence.id}><div testId={`scheduled-work-${occurrence.id}`} style={{ display: "flex", flexDirection: "column", gap: 5, padding: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 7, backgroundColor: palette.panel }}>
        <text style={{ color: palette.text, fontSize: 12 }}>{occurrence.state.kind}</text>
        <text style={{ color: palette.textMuted, fontSize: 10 }}>{`Due ${occurrence.dueAtMs}`}</text>
        <div style={{ display: "flex", gap: 6 }}>
          {runId && <div testId={`open-scheduled-work-run-${occurrence.id}`} tabIndex={0} accessibilityRole="button" accessibilityName="Open scheduled work run" onClick={() => onOpenRun(runId)} onKeyDown={(event) => { if (activates(event)) onOpenRun(runId) }} style={actionStyle(true)}><text>Open run</text></div>}
          <div testId={`cancel-scheduled-work-${occurrence.id}`} tabIndex={cancelEnabled ? 0 : -1} accessibilityRole="button" accessibilityName="Cancel scheduled work" accessibilityDisabled={!cancelEnabled} onClick={() => { if (cancelEnabled) scheduledWork.cancel(occurrence.id) }} onKeyDown={(event) => { if (activates(event) && cancelEnabled) scheduledWork.cancel(occurrence.id) }} style={actionStyle(cancelEnabled)}><text>Cancel</text></div>
        </div>
      </div></Fragment>
    })}
  </div>
}
