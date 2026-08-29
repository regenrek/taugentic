import { Fragment } from "react"

import { palette } from "../../app/theme.js"
import { Pressable } from "../../ui/pressable.js"
import type { WorkInboxState } from "./use-work-inbox.js"

function actionStyle(enabled: boolean) {
  return { cursor: enabled ? "pointer" : "default", padding: 6, backgroundColor: enabled ? palette.panelRaised : palette.panel, opacity: enabled ? 1 : 0.6 }
}

/** Sidebar navigation surface for daemon-projected, non-scheduled WorkItems. */
export function WorkInboxPanel({ inbox, canTrigger }: { inbox: WorkInboxState; canTrigger: boolean }) {
  const refreshEnabled = inbox.actionsEnabled && !inbox.busy
  return <div testId="work-inbox" style={{ display: "flex", flexDirection: "column", gap: 7, paddingTop: 8 }}>
    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
      <text style={{ color: palette.textFaint, fontSize: 10, fontWeight: 700 }}>WORK INBOX</text><div style={{ flexGrow: 1 }} />
      <Pressable testId="refresh-work-inbox" name="Refresh Work Inbox" disabled={!refreshEnabled} onPress={inbox.refresh} style={actionStyle(refreshEnabled)}><text style={{ fontSize: 10 }}>Refresh</text></Pressable>
    </div>
    {inbox.sync && <text testId="work-inbox-sync" style={{ color: palette.textMuted, fontSize: 10 }}>{inbox.sync.state}{inbox.sync.detail ? ` · ${inbox.sync.detail}` : ""}</text>}
    {inbox.error && <text testId="work-inbox-error" style={{ color: "#f08080", fontSize: 11 }}>{inbox.error}</text>}
    {inbox.mutationError && <text testId="work-inbox-mutation-error" style={{ color: "#f08080", fontSize: 11 }}>{inbox.mutationError}</text>}
    {inbox.loading && !inbox.items.length && <text testId="work-inbox-loading" style={{ color: palette.textMuted, fontSize: 11 }}>Loading work items…</text>}
    {!inbox.loading && !inbox.items.length && <text testId="work-inbox-empty" style={{ color: palette.textMuted, fontSize: 11 }}>No work items available.</text>}
    {inbox.items.map((item) => {
      const triggerEnabled = !inbox.busy && canTrigger && item.status === "available"
      const dismissEnabled = !inbox.busy && inbox.actionsEnabled
      return <Fragment key={item.key}><div testId={`work-item-${item.key}`} style={{ display: "flex", flexDirection: "column", gap: 5, padding: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 7, backgroundColor: palette.panel }}>
      <text style={{ color: palette.text, fontSize: 12, fontWeight: 600 }}>{item.title}</text>
      <text style={{ color: palette.textMuted, fontSize: 10 }}>{item.status}{item.labels.length ? ` · ${item.labels.join(", ")}` : ""}</text>
      <div style={{ display: "flex", gap: 6 }}>
        <Pressable testId={`trigger-work-item-${item.key}`} name={`Run work item ${item.title}`} disabled={!triggerEnabled} onPress={() => inbox.trigger(item)} style={actionStyle(triggerEnabled)}><text style={{ fontSize: 10 }}>Run</text></Pressable>
        <Pressable testId={`dismiss-work-item-${item.key}`} name={`Dismiss work item ${item.title}`} disabled={!dismissEnabled} onPress={() => inbox.dismiss(item.key)} style={actionStyle(dismissEnabled)}><text style={{ fontSize: 10 }}>Dismiss</text></Pressable>
      </div>
    </div></Fragment>
    })}
  </div>
}
