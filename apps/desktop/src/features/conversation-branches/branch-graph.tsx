import type { RunLineageGraphResult, RunListEntry } from "@taugentic/desktop-protocol"
import { Fragment, useState } from "react"
import { fontSize, palette } from "../../app/theme.js"
import { Pressable } from "../../ui/pressable.js"

type CortexState = "loading" | "offline" | "error" | "ready"
export type CortexCanvasCommand =
  | { type: "line"; id: string; from: { x: number; y: number }; to: { x: number; y: number }; width: number; color: string }
  | { type: "circle"; id: string; center: { x: number; y: number }; radius: number; color: string }

/** Maps the daemon-owned bounded projection into the passive native draw list. */
export function mapCortexGraph(graph: RunLineageGraphResult): { nodes: RunListEntry[]; commands: CortexCanvasCommand[] } {
  const nodes = graph.nodes.slice(0, 128)
  const positions = new Map(nodes.map((node, index) => [node.id, { x: 0.08 + (index % 8) * 0.12, y: 0.15 + Math.floor(index / 8) * 0.12 }]))
  const commands = [...graph.edges.slice(0, 127).flatMap((edge) => {
    const from = positions.get(edge.parentRunId)
    const to = positions.get(edge.childRunId)
    return from && to ? [{ type: "line" as const, id: `edge-${edge.parentRunId}-${edge.childRunId}`, from, to, width: 0.008, color: palette.border }] : []
  }), ...nodes.map((node) => ({ type: "circle" as const, id: `node-${node.id}`, center: positions.get(node.id)!, radius: 0.025, color: node.status === "failed" ? "#f08080" : node.status === "running" ? palette.accent : palette.textMuted }))].slice(0, 256)
  return { nodes, commands }
}

function label(node: RunListEntry): string {
  if (node.relationship.kind === "root") return "Root run"
  if (node.relationship.kind === "nativeSubagent") return "Subagent"
  if (node.relationship.kind === "freshSpawn") return "Fresh Spawn"
  if (node.relationship.kind === "fork") return `Side Chat at turn ${node.relationship.parentEventSeq}`
  return `Route switch at turn ${node.relationship.parentEventSeq}`
}

function routeIdentity(node: RunListEntry): string | undefined {
  if (node.relationship.kind !== "routeSwitchedContinuation") return undefined
  const { route } = node.relationship
  return `Provider: ${route.providerId} · Harness: ${route.harness} · Model: ${route.modelId ?? "not selected"} · Auth profile: ${route.authProfileId ?? "not selected"}`
}

/** Passive Canvas picture; the adjacent tree is the only semantic interaction surface. */
export function ConversationBranchGraph(props: { graph?: RunLineageGraphResult; state?: CortexState; visible: boolean; onOpen(runId: string): void }) {
  const graph = props.graph
  const [selectedRunId, setSelectedRunId] = useState<string>()
  if (props.state === "loading") return <text testId="cortex-loading" style={{ color: palette.textMuted }}>Loading Cortex…</text>
  if (props.state === "offline") return <text testId="cortex-offline" style={{ color: palette.warning }}>Cortex is unavailable while the daemon is offline.</text>
  if (props.state === "error") return <text testId="cortex-error" style={{ color: "#f08080" }}>Cortex could not be loaded.</text>
  if (!graph?.nodes.length) return <text testId="cortex-empty" style={{ color: palette.textMuted }}>No runs in this conversation.</text>
  const { nodes, commands } = mapCortexGraph(graph)
  const open = (runId: string) => { setSelectedRunId(runId); props.onOpen(runId) }
  return <div testId="cortex" style={{ display: "flex", flexDirection: "column", gap: 8, padding: 10, borderWidth: 1, borderColor: palette.accentDim, borderRadius: 8, backgroundColor: palette.panelRaised }}>
    <text style={{ color: palette.text, fontSize: fontSize(11), fontWeight: 650 }}>Cortex</text>
    <canvas testId="cortex-canvas" visible={props.visible} motion="paused" commands={commands} style={{ width: 288, height: 164 }} />
    {graph.truncated && <text testId="cortex-truncated" style={{ color: palette.warning, fontSize: fontSize(10) }}>Showing {nodes.length} of {graph.totalCount} runs.</text>}
    {!!graph.orphanRunIds.length && <text testId="cortex-orphans" style={{ color: palette.warning, fontSize: fontSize(10) }}>Missing parent for {graph.orphanRunIds.length} run(s).</text>}
    {graph.cycleBroken && <text testId="cortex-cycle" style={{ color: palette.warning, fontSize: fontSize(10) }}>A lineage cycle was safely broken.</text>}
    <div testId="conversation-branch-graph" accessibilityRole="tree" accessibilityName="Cortex run tree" style={{ display: "flex", flexDirection: "column", gap: 3 }}>{nodes.map((node) => <Fragment key={node.id}><Pressable testId={`branch-node-${node.id}`} role="treeitem" name={`${label(node)} ${node.id}. Open`} selected={selectedRunId === node.id} onPress={() => open(node.id)} style={{ display: "flex", flexDirection: "column", gap: 3, cursor: "pointer", padding: 5, borderRadius: 5, backgroundColor: selectedRunId === node.id ? palette.panelRaised : palette.panel }}><div style={{ display: "flex", gap: 6 }}><text style={{ color: palette.textMuted, fontSize: fontSize(10) }}>{label(node)} · {node.id}</text><text style={{ color: palette.textFaint, fontSize: fontSize(10) }}>{node.status}</text></div>{routeIdentity(node) && <text testId={`branch-route-${node.id}`} style={{ color: palette.textFaint, fontSize: fontSize(10) }}>{routeIdentity(node)}</text>}</Pressable></Fragment>)}</div>
  </div>
}
