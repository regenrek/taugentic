import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"
import React from "react"
import type { RunLineageGraphResult } from "@taugentic/desktop-protocol"
import { ConversationBranchGraph } from "../src/features/conversation-branches/branch-graph.js"
import { isDockPanelVisible } from "../src/features/workspace-layout/layout-store.js"

const graph: RunLineageGraphResult = { nodes: [{ id: "root", relationship: { kind: "root" }, harness: "native", status: "completed" }, { id: "fork", relationship: { kind: "fork", parentRunId: "root", parentEventSeq: "4" }, harness: "native", status: "running" }], edges: [{ parentRunId: "root", childRunId: "fork" }], orphanRunIds: [], totalCount: 2, omittedCount: 0, truncated: false, cycleBroken: false }

describe("M8 Cortex", () => {
  it("derives Canvas visibility from the persisted active or zoomed conversation dock panel", () => {
    const layout = { kind: "split" as const, id: "root", direction: "horizontal" as const, ratio: 0.5, first: { kind: "tabs" as const, id: "left", panels: ["conversation", "files"], active: "files" }, second: { kind: "tabs" as const, id: "right", panels: ["git"], active: "git" } }
    expect(isDockPanelVisible(layout, "conversation")).toBe(false)
    expect(isDockPanelVisible({ ...layout, first: { ...layout.first, active: "conversation" } }, "conversation")).toBe(true)
    expect(isDockPanelVisible({ ...layout, zoomed: "conversation" }, "conversation")).toBe(true)
  })
  it("uses one static Canvas and native pointer, Enter, Space, focus, and selected tree semantics", () => {
    const root = createTestRoot(); const opened: string[] = []
    try {
      root.render(<ConversationBranchGraph graph={graph} state="ready" visible onOpen={(id) => opened.push(id)} />)
      const node = root.renderer.findByTestId("branch-node-fork")!
      const canvas = root.renderer.findByTestId("cortex-canvas")!
      expect(canvas.customProps?.visible).toBe(true)
      expect(root.renderer.getAutomationTree()).toContain("Cortex run tree")
      expect(root.renderer.getAutomationTree()).toContain("Side Chat at turn 4 fork. Open")
      const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(node.id)!
      root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
      root.renderer.nativeSimulateKeystrokes(node.id, "enter space")
      root.renderer.focusElement(node.id)
      root.renderer.simulateKeystrokes("enter")
      expect(opened).toEqual(["fork", "fork", "fork", "fork"])
      expect(root.renderer.getAutomationTree()).toContain("selected")
    } finally { root.unmount() }
  })
  it("hides the passive Canvas from the native tree when the persisted dock panel is inactive", () => {
    const root = createTestRoot()
    try { root.render(<ConversationBranchGraph graph={graph} state="ready" visible={false} onOpen={() => {}} />); expect(root.renderer.findByTestId("cortex-canvas")?.customProps?.visible).toBe(false) } finally { root.unmount() }
  })
  it("renders all bounded product states", () => {
    const root = createTestRoot()
    try { for (const state of ["loading", "offline", "error"] as const) { root.render(<ConversationBranchGraph state={state} visible onOpen={() => {}} />); expect(root.renderer.findByTestId(`cortex-${state}`)).toBeDefined() }; root.render(<ConversationBranchGraph graph={{ ...graph, nodes: [] }} state="ready" visible onOpen={() => {}} />); expect(root.renderer.findByTestId("cortex-empty")).toBeDefined() } finally { root.unmount() }
  })
})
