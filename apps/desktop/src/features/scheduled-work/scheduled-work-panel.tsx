import { Fragment } from "react"

import { fontSize, palette } from "../../app/theme.js"
import { Pressable } from "../../ui/pressable.js"
import type { ScheduledWorkState } from "./use-scheduled-work.js"

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
    <text style={{ color: palette.textFaint, fontSize: fontSize(10), fontWeight: 700 }}>SCHEDULED WORK</text>
    <input testId="scheduled-work-objective" accessibilityName="Work objective" value={scheduledWork.objective} placeholder="Work objective" onChange={(event) => scheduledWork.setObjective(event.value ?? "")} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
    <input testId="scheduled-work-due-at-ms" accessibilityName="Due time in Unix milliseconds" value={scheduledWork.dueAtMs} placeholder="Due time (Unix ms)" onChange={(event) => scheduledWork.setDueAtMs(event.value ?? "")} onSubmit={() => scheduledWork.create()} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
    <Pressable testId="create-scheduled-work" name="Schedule work" disabled={!scheduledWork.canCreate} onPress={scheduledWork.create} style={actionStyle(scheduledWork.canCreate)}><text>Schedule work</text></Pressable>
    {scheduledWork.error && <text testId="scheduled-work-error" accessibilityRole="alert" style={{ color: "#f08080", fontSize: fontSize(11) }}>{scheduledWork.error}</text>}
    {scheduledWork.mutationError && <text testId="scheduled-work-mutation-error" accessibilityRole="alert" style={{ color: "#f08080", fontSize: fontSize(11) }}>{scheduledWork.mutationError}</text>}
    {scheduledWork.loading && !scheduledWork.occurrences.length && <text testId="scheduled-work-loading" style={{ color: palette.textMuted, fontSize: fontSize(11) }}>Loading scheduled work…</text>}
    {!scheduledWork.loading && !scheduledWork.occurrences.length && <text testId="scheduled-work-empty" style={{ color: palette.textMuted, fontSize: fontSize(11) }}>No scheduled work.</text>}
    {scheduledWork.occurrences.map((occurrence) => {
      const runId = runIdFor(occurrence.state)
      const cancelEnabled = !scheduledWork.busy && isCancellable(occurrence.state)
      return <Fragment key={occurrence.id}><div testId={`scheduled-work-${occurrence.id}`} style={{ display: "flex", flexDirection: "column", gap: 5, padding: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 7, backgroundColor: palette.panel }}>
        <text style={{ color: palette.text, fontSize: fontSize(12) }}>{occurrence.state.kind}</text>
        <text style={{ color: palette.textMuted, fontSize: fontSize(10) }}>{`Due ${occurrence.dueAtMs}`}</text>
        <div style={{ display: "flex", gap: 6 }}>
          {runId && <Pressable testId={`open-scheduled-work-run-${occurrence.id}`} name="Open scheduled work run" onPress={() => onOpenRun(runId)} style={actionStyle(true)}><text>Open run</text></Pressable>}
          <Pressable testId={`cancel-scheduled-work-${occurrence.id}`} name="Cancel scheduled work" disabled={!cancelEnabled} onPress={() => scheduledWork.cancel(occurrence.id)} style={actionStyle(cancelEnabled)}><text>Cancel</text></Pressable>
        </div>
      </div></Fragment>
    })}
  </div>
}
