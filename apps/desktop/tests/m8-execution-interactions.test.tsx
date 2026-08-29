import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import { ConversationBranchGraph } from "../src/features/conversation-branches/branch-graph.js"

describe("M8 execution interaction semantics", () => {
  it("renders Cortex run choices as selected treeitems through the shared Pressable surface", () => {
    const root = createTestRoot()
    const opened: string[] = []
    try {
      root.render(<ConversationBranchGraph
        visible
        onOpen={(runId) => opened.push(runId)}
        graph={{
          nodes: [{ id: "run-child", relationship: { kind: "freshSpawn", parentRunId: "run-parent" }, status: "running", harness: "native" }],
          edges: [], orphanRunIds: [], totalCount: 1, omittedCount: 0, truncated: false, cycleBroken: false,
        }}
      />)
      const node = root.renderer.findByTestId("branch-node-run-child")!
      const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(node.id) ?? []
      root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)

      expect(opened).toEqual(["run-child"])
      expect(root.renderer.getAutomationTree()).toContain('"role":"treeitem"')
      expect(root.renderer.getAutomationTree()).toContain('"selected":true')
    } finally {
      root.unmount()
    }
  })
})
