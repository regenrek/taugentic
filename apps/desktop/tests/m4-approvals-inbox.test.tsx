import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import { approvalTargetLabel, ApprovalsInboxPanel } from "../src/features/approvals/approvals-inbox-panel.js"

function click(root: ReturnType<typeof createTestRoot>, testId: string) {
  const element = root.renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(element.id) ?? []
  root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

describe("M4 approvals inbox", () => {
  it("presents daemon-provided approval facts and dispatches only explicit decisions", () => {
    const opened: string[] = []
    const decisions: string[] = []
    const root = createTestRoot()
    try {
      root.render(<div style={{ width: 700, height: 500 }}><ApprovalsInboxPanel inbox={{
        approvals: [{ id: "approval-one", runId: "run-origin", scope: "processExec", requestedAtMs: "1", expiresAtMs: "999", target: { kind: "processExec", command: "cargo fmt --check" }, reason: "Run formatter" }],
        loading: false,
        error: undefined,
        decide: async (id, decision) => { decisions.push(`${id}:${decision}`) },
      }} onOpenRun={(runId) => opened.push(runId)} /></div>)
      const rendered = root.renderer.getAllText().join(" ")
      expect(rendered).toContain("Scope:")
      expect(rendered).toContain("Target:")
      expect(rendered).toContain("processExec")
      expect(rendered).toContain("Command:")
      expect(rendered).toContain("cargo fmt --check")
      expect(rendered).toContain("Expires:")
      expect(rendered).toContain("999")
      expect(rendered).toContain("Originating run:")
      expect(rendered).toContain("run-origin")
      click(root, "open-approval-run-approval-one")
      click(root, "approve-inbox-approval-one")
      click(root, "reject-inbox-approval-one")
      expect(opened).toEqual(["run-origin"])
      expect(decisions).toEqual(["approval-one:approved", "approval-one:rejected"])
    } finally {
      root.unmount()
    }
  })

  it("renders every daemon target shape without inferring missing facts", () => {
    expect(approvalTargetLabel({ kind: "toolCall", toolName: "read_file" })).toBe("toolCall · Tool: read_file")
    expect(approvalTargetLabel({ kind: "fileWrite", paths: ["src/main.rs", "Cargo.toml"] })).toBe("fileWrite · Paths: src/main.rs, Cargo.toml")
    expect(approvalTargetLabel({ kind: "processExec", command: null })).toBe("processExec")
    expect(approvalTargetLabel({ kind: "networkAccess", protocol: "https", host: "example.com" })).toBe("networkAccess · Protocol: https · Host: example.com")
    expect(approvalTargetLabel({ kind: "capsuleDispatch", childRunId: "run-child", workspaceScope: "workspaceWrite" })).toBe("capsuleDispatch · Child run: run-child · Workspace scope: workspaceWrite")
  })
})
