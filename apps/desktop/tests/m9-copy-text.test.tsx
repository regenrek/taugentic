import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import type { AgentTurnRow } from "@taugentic/desktop-protocol"

import { ApprovalsInboxPanel } from "../src/features/approvals/approvals-inbox-panel.js"
import { DiffPanel } from "../src/features/files/file-panels.js"
import { RunActivityPanel } from "../src/features/run-activity/run-activity-panel.js"
import { createCommandDispatcher } from "../src/features/commands/registry.js"
import { ConversationPanel, type ConversationPanelProps } from "../src/features/workspace-layout/panels.js"
import { DesktopSettings } from "../src/platform/settings/desktop-settings.js"

function click(root: ReturnType<typeof createTestRoot>, testId: string) {
  const element = root.renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(element.id) ?? []
  root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

function conversationProps(rows: readonly AgentTurnRow[], copyText: (text: string) => void): ConversationPanelProps {
  return {
    title: "Copy transcript",
    selectedConversationId: "session-copy",
    transcriptRows: rows,
    transcriptLoading: false,
    hasOlderTranscript: false,
    loadingOlderTranscript: false,
    onLoadOlderTranscript() {},
    messages: [],
    objective: "",
    attachments: [],
    onObjectiveChange() {},
    onRemoveAttachment() {},
    copyText,
    commands: createCommandDispatcher(new DesktopSettings(), () => ({ canStart: false, canCancel: false }), { openSettings() {}, openBrowser() {}, focusPanel() {}, toggleTheme() {}, startRun() {}, cancelRun() {} }),
  }
}

describe("M9 copy text", () => {
  it("copies exact durable transcript and tool payloads only after their explicit native control is activated", () => {
    const root = createTestRoot()
    const rows: AgentTurnRow[] = [
      { kind: "user", cursor: { sequence: "1" }, sessionId: "session-copy", runId: "run-copy", occurredAtMs: "1", text: "user durable text", attachments: [] },
      { kind: "assistant", cursor: { sequence: "2" }, sessionId: "session-copy", runId: "run-copy", turnId: "turn-copy", startedAtMs: "2", completedAtMs: "2", text: "assistant durable text" },
      { kind: "toolCall", cursor: { sequence: "3" }, sessionId: "session-copy", runId: "run-copy", turnId: "turn-copy", itemId: "tool-copy", toolName: "shell", input: "{\"cmd\":\"pwd\"}", output: "/workspace", outcome: "completed", startedAtMs: "3", completedAtMs: "3" },
      { kind: "toolCall", cursor: { sequence: "4" }, sessionId: "session-copy", runId: "run-copy", turnId: "turn-copy", itemId: "tool-empty", toolName: "noop", input: "", output: "", outcome: "completed", startedAtMs: "4", completedAtMs: "4" },
    ]
    try {
      root.render(<div style={{ width: 1000, height: 900 }}><ConversationPanel {...conversationProps(rows, (text) => root.renderer.writeClipboardText(text))} /></div>)
      expect(root.renderer.takeClipboardWrites()).toEqual([])
      expect(root.renderer.findByTestId("copy-tool-input-4")).toBeUndefined()
      expect(root.renderer.findByTestId("copy-tool-output-4")).toBeUndefined()
      click(root, "copy-transcript-1")
      click(root, "copy-transcript-2")
      click(root, "copy-tool-input-3")
      click(root, "copy-tool-output-3")
      expect(root.renderer.takeClipboardWrites()).toEqual(["user durable text", "assistant durable text", "{\"cmd\":\"pwd\"}", "/workspace"])
    } finally {
      root.unmount()
    }
  })

  it("copies exactly the selected durable run objective, patch, and approval reasons through the injected renderer", () => {
    const root = createTestRoot()
    const activity = {
      runs: [], selectedRunId: "run-copy", selectRun() {},
      detail: { summary: { id: "run-copy", runtimeProfileId: "profile-copy", objective: "Run objective", status: "waitingForApproval" }, executionContext: {} },
      timeline: undefined, replay: [], activity: [],
      approvals: [{ id: "approval-detail", runId: "run-copy", scope: "processExec", requestedAtMs: "1", target: { kind: "processExec" }, reason: "Detail approval reason" }],
      loading: false, loadingOlderActivity: false, loadOlderActivity() {}, hasOlderActivity: false,
      loadingOlderRuns: false, loadOlderRuns() {}, hasOlderRuns: false,
      error: undefined, refresh() {}, cancel: async () => {}, switchEligible: false, switchAccountAndResume: async () => {}, openArtifact() {}, decide: async () => {},
    } as never
    const copyText = (text: string) => root.renderer.writeClipboardText(text)
    try {
      root.render(<div style={{ width: 1000, height: 900 }}><RunActivityPanel activity={activity} copyText={copyText} /></div>)
      expect(root.renderer.takeClipboardWrites()).toEqual([])
      click(root, "copy-run-objective")
      click(root, "copy-approval-approval-detail")
      root.render(<div style={{ width: 1000, height: 900 }}><DiffPanel label="changes.diff" content={{ kind: "text", text: "diff --git a/a b/a\n+line", revision: "rev", byteLen: "24" }} loading={false} pdfPageIndex={0} setPdfPageIndex={() => {}} copyText={copyText} /></div>)
      click(root, "copy-selected-diff")
      root.render(<div style={{ width: 1000, height: 900 }}><ApprovalsInboxPanel inbox={{ approvals: [{ id: "approval-inbox", runId: "run-copy", scope: "fileWrite", requestedAtMs: "2", expiresAtMs: "3", target: { kind: "fileWrite", paths: ["src/a.ts"] }, reason: "Inbox approval reason" }], loading: false, error: undefined, decide: async () => {} }} onOpenRun={() => {}} copyText={copyText} /></div>)
      click(root, "copy-inbox-approval-approval-inbox")
      expect(root.renderer.takeClipboardWrites()).toEqual(["Run objective", "Detail approval reason", "diff --git a/a b/a\n+line", "Inbox approval reason"])
    } finally {
      root.unmount()
    }
  })
})
