import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import { ArtifactsPanel } from "../src/features/artifacts/artifacts-panel.js"
import { ScheduledWorkPanel } from "../src/features/scheduled-work/scheduled-work-panel.js"
import { ThreadWorkspacePanel } from "../src/features/thread-workspace/thread-workspace-panel.js"
import { WorkInboxPanel } from "../src/features/work-items/work-inbox-panel.js"

function click(root: ReturnType<typeof createTestRoot>, testId: string) {
  const element = root.renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(element.id) ?? []
  root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

describe("M8 remaining panel interaction semantics", () => {
  it("activates representative standalone panel controls through click, Enter, and Space", () => {
    const calls: string[] = []
    const root = createTestRoot()
    try {
      root.render(<div style={{ width: 900, height: 700 }}>
        <ArtifactsPanel artifacts={[{ id: "artifact-one", runId: "run-one", kind: "Image", displayName: "chart.png", metadata: { kind: "image", mediaType: "png", sha256: "hash", byteLen: "1", provenance: { runtimeProfileId: "profile", providerId: "provider", turnId: "turn", itemId: "item" } } }]} loading={false} selectArtifact={() => calls.push("select")} openImageArtifact={() => calls.push("open")} refresh={() => calls.push("refresh-artifacts")} />
        <ScheduledWorkPanel scheduledWork={{ objective: "", dueAtMs: "", occurrences: [], loading: false, busy: false, canCreate: true, setObjective() {}, setDueAtMs() {}, create: () => calls.push("schedule"), cancel() {} } as never} onOpenRun={() => {}} />
        <WorkInboxPanel inbox={{ items: [], loading: false, busy: false, actionsEnabled: false, refresh: () => calls.push("refresh-inbox"), dismiss() {}, trigger() {} } as never} canTrigger={false} />
        <ThreadWorkspacePanel workspace={{ sessionId: "session-one", projection: { pins: [], workLog: [] }, loading: false, drafts: { goal: "", plan: "", recap: "", notes: "" }, dirty: { goal: false, plan: false, recap: false, notes: false }, busy: false, setDraft() {}, save: () => calls.push("save"), addPin() {}, removePin() {}, refresh: () => calls.push("refresh-thread") } as never} />
      </div>)
      click(root, "refresh-artifacts")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("open-image-artifact-artifact-one")!.id, "enter")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("create-scheduled-work")!.id, "space")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("refresh-thread-workspace")!.id, "enter")
      const disabled = root.renderer.findByTestId("refresh-work-inbox")!
      click(root, "refresh-work-inbox")
      root.renderer.nativeSimulateKeystrokes(disabled.id, "enter space")
      expect(calls).toEqual(["refresh-artifacts", "open", "select", "schedule", "refresh-thread"])
      expect(root.renderer.getAutomationTree()).toContain('"testId":"refresh-work-inbox","accessibility":{"role":"button","name":"Refresh Work Inbox","disabled":true}')
    } finally {
      root.unmount()
    }
  })
})
