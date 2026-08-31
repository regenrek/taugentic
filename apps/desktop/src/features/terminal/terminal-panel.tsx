import type { EventPayload } from "@regenrek/gpuix-react"
import { Fragment, useCallback } from "react"

import { fontSize, palette } from "../../app/theme.js"
import { Pressable } from "../../ui/pressable.js"
import type { WorkbenchTerminalState } from "./use-workbench-terminal.js"

export function TerminalPanel({ terminal }: { terminal: WorkbenchTerminalState }) {
  const surfaceRef = useCallback((instance: { id: number } | null) => {
    terminal.setTerminalSurface(instance?.id)
  }, [terminal.setTerminalSurface])

  return <div testId="terminal-panel" style={{ display: "flex", flexDirection: "column", width: "100%", height: "100%", minWidth: 0, minHeight: 0, backgroundColor: palette.canvas }}>
    <div style={{ display: "flex", alignItems: "center", gap: 8, padding: 8, borderBottomWidth: 1, borderColor: palette.border }}>
      <Pressable testId="new-terminal" name="New terminal" disabled={!terminal.canSpawn} onPress={() => { void terminal.spawn() }} style={{ padding: 7, borderRadius: 6, backgroundColor: terminal.canSpawn ? palette.accentDim : palette.panelRaised, color: terminal.canSpawn ? palette.text : palette.textFaint, cursor: terminal.canSpawn ? "pointer" : "default" }}><text>New terminal</text></Pressable>
      <div style={{ display: "flex", flexGrow: 1, minWidth: 0, gap: 4, overflow: "scroll" }}>
        {terminal.terminals.map((session, index) => <Fragment key={session.id}><div style={{ display: "flex", alignItems: "center", gap: 4, flexShrink: 0 }}>
          <Pressable testId={`select-terminal-${session.id}`} name={`Terminal ${index + 1}`} role="tab" selected={session.id === terminal.selectedTerminalId} onPress={() => terminal.selectTerminal(session.id)} style={{ padding: 7, borderRadius: 6, backgroundColor: session.id === terminal.selectedTerminalId ? palette.panelRaised : palette.panel, color: session.status === "running" ? palette.text : palette.textMuted, cursor: "pointer" }}><text>{`Terminal ${index + 1}${session.status === "exited" ? " · exited" : ""}`}</text></Pressable>
          <Pressable testId={`close-terminal-${session.id}`} name={`Close Terminal ${index + 1}`} disabled={terminal.busy || session.status !== "running"} onPress={() => { void terminal.close(session.id) }} style={{ padding: 5, color: palette.textMuted, cursor: !terminal.busy && session.status === "running" ? "pointer" : "default" }}><text>×</text></Pressable>
        </div></Fragment>)}
      </div>
      {terminal.viewport && <text testId="terminal-viewport" style={{ color: palette.textFaint, fontSize: fontSize(10) }}>{`${terminal.viewport.cols}×${terminal.viewport.rows}`}</text>}
    </div>
    {terminal.error && <text testId="terminal-error" style={{ color: "#f08080", fontSize: fontSize(11), padding: 7 }}>{terminal.error}</text>}
    {terminal.snapshotTruncated && <text testId="terminal-history-truncated" style={{ color: palette.warning, fontSize: fontSize(10), padding: 7 }}>Earlier terminal output is not available.</text>}
    <terminal
      ref={surfaceRef}
      testId="terminal-surface"
      tabIndex={0}
      onTerminalInput={(event: EventPayload) => terminal.sendInput(event.dataBase64)}
      onTerminalResize={(event: EventPayload) => terminal.resize(event.rows, event.cols)}
      style={{ flexGrow: 1, minWidth: 0, minHeight: 0, width: "100%", height: "100%" }}
    />
    {!terminal.selectedTerminal && <div style={{ position: "absolute", left: 18, bottom: 18 }}><text style={{ color: palette.textFaint, fontSize: fontSize(11) }}>{terminal.viewport ? "Create or select a terminal session." : "Measuring terminal viewport…"}</text></div>}
  </div>
}
