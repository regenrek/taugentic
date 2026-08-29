import { Fragment } from "react"

import { palette } from "../../app/theme.js"
import { Pressable } from "../../ui/pressable.js"
import type { ThreadWorkspacePanelState } from "./use-thread-workspace.js"

const fields = [
  ["goal", "Goal"],
  ["plan", "Plan"],
  ["recap", "Recap"],
  ["notes", "Notes"],
] as const

/** One scroll surface for durable thread context; all content is daemon-projected. */
export function ThreadWorkspacePanel({ workspace }: { workspace: ThreadWorkspacePanelState }) {
  if (!workspace.sessionId) return <div testId="thread-workspace-panel" style={centered}><text style={muted}>Select a conversation to view its thread workspace.</text></div>
  if (workspace.loading && !workspace.projection) return <div testId="thread-workspace-panel" style={centered}><text style={muted}>Loading thread workspace…</text></div>
  if (workspace.error && !workspace.projection) return <div testId="thread-workspace-panel" style={centered}><text testId="thread-workspace-error" style={errorText}>{workspace.error}</text></div>

  return <div testId="thread-workspace-panel" style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, backgroundColor: palette.canvas }}>
    <div style={{ display: "flex", alignItems: "center", minHeight: 40, paddingLeft: 12, paddingRight: 8, borderBottomWidth: 1, borderColor: palette.border }}>
      <text style={{ color: palette.text, fontSize: 12, fontWeight: 650 }}>Thread workspace</text><div style={{ flexGrow: 1 }} />
      <Pressable testId="refresh-thread-workspace" name="Refresh thread workspace" disabled={workspace.busy} onPress={workspace.refresh} style={actionStyle(!workspace.busy)}><text style={{ fontSize: 10 }}>Refresh</text></Pressable>
    </div>
    <div style={{ display: "flex", flexDirection: "column", flexGrow: 1, minHeight: 0, overflow: "scroll", padding: 12, gap: 14 }}>
      {workspace.error && <text testId="thread-workspace-error" style={errorText}>{workspace.error}</text>}
      {workspace.mutationError && <text testId="thread-workspace-mutation-error" style={errorText}>{workspace.mutationError}</text>}
      {fields.map(([field, label]) => <Fragment key={field}><div style={{ display: "flex", flexDirection: "column", gap: 7 }}>
        <div style={{ display: "flex", alignItems: "center" }}><text style={{ color: palette.text, fontSize: 12, fontWeight: 600 }}>{label}</text><div style={{ flexGrow: 1 }} />
          <Pressable testId={`save-thread-workspace-${field}`} name={`Save ${label}`} disabled={workspace.busy} onPress={() => workspace.save(field)} style={actionStyle(!workspace.busy)}><text style={{ fontSize: 10 }}>Save</text></Pressable>
        </div>
        <textarea testId={`thread-workspace-${field}`} value={workspace.drafts[field]} minRows={field === "notes" ? 5 : 3} maxRows={14} onChange={(event) => workspace.setDraft(field, event.value ?? "")} placeholder={`Add ${label.toLowerCase()}…`} style={{ minHeight: field === "notes" ? 110 : 72, padding: 9, borderWidth: 1, borderColor: palette.border, borderRadius: 7, color: palette.text, backgroundColor: palette.panel }} />
      </div></Fragment>)}
      <div style={{ display: "flex", flexDirection: "column", gap: 7 }}>
        <text style={{ color: palette.text, fontSize: 12, fontWeight: 600 }}>Pinned turns</text>
        {!workspace.projection?.pins.length && <text testId="thread-workspace-pins-empty" style={muted}>No pinned durable turns.</text>}
        {workspace.projection?.pins.map((pin) => <Fragment key={`${pin.runId}:${pin.cursor.sequence}`}><div testId={`thread-workspace-pin-${pin.runId}-${pin.cursor.sequence}`} style={{ display: "flex", alignItems: "center", gap: 8, padding: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 7, backgroundColor: palette.panel }}>
          <text style={{ color: palette.textMuted, fontSize: 10, userSelect: "text" }}>{`${pin.runId} · ${pin.cursor.sequence}`}</text><div style={{ flexGrow: 1 }} />
          <Pressable testId={`remove-thread-workspace-pin-${pin.runId}-${pin.cursor.sequence}`} name={`Remove pinned turn ${pin.cursor.sequence}`} disabled={workspace.busy} onPress={() => workspace.removePin(pin.cursor.sequence)} style={actionStyle(!workspace.busy)}><text style={{ fontSize: 10 }}>Remove</text></Pressable>
        </div></Fragment>)}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 7 }}>
        <text style={{ color: palette.text, fontSize: 12, fontWeight: 600 }}>Work log</text>
        {!workspace.projection?.workLog.length && <text testId="thread-workspace-work-log-empty" style={muted}>No durable thread updates yet.</text>}
        {workspace.projection?.workLog.map((entry) => <Fragment key={entry.sequence}><div testId={`thread-work-log-${entry.sequence}`} style={{ display: "flex", gap: 8, padding: 8, borderLeftWidth: 2, borderColor: palette.accentDim }}><text style={{ color: palette.textFaint, fontSize: 10 }}>{entry.sequence}</text><text style={{ color: palette.textMuted, fontSize: 10 }}>{entry.kind}</text></div></Fragment>)}
      </div>
    </div>
  </div>
}

const centered = { display: "flex", flexGrow: 1, alignItems: "center", justifyContent: "center" } as const
const muted = { color: palette.textMuted, fontSize: 12 } as const
const errorText = { color: "#f08080", fontSize: 11 } as const
function actionStyle(enabled: boolean) {
  return { padding: 6, borderRadius: 5, cursor: enabled ? "pointer" : "default", backgroundColor: enabled ? palette.panelRaised : palette.panel, color: enabled ? palette.text : palette.textFaint }
}
