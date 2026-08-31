import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import type { AgentTurnRow, WorkspaceFileAttachment } from "@taugentic/desktop-protocol"

import { panelRegistry, type ConversationPanelProps, type WorkbenchPanelProps } from "../src/features/workspace-layout/panels.js"
import { createCommandDispatcher } from "../src/features/commands/registry.js"
import { transcriptHasCommittedAssistant, transcriptRows } from "../src/platform/daemon/transcript-query.js"
import { DesktopSettings } from "../src/platform/settings/desktop-settings.js"
import { codeHostState } from "./support/code-host.js"

function user(sequence: number, text = `request ${sequence}`, attachments: WorkspaceFileAttachment[] = []): AgentTurnRow {
  return { kind: "user", cursor: { sequence: String(sequence) }, sessionId: "session-one", runId: `run-${sequence}`, occurredAtMs: String(sequence), text, attachments }
}

function assistant(sequence: number, text: string, turnId = `turn-${sequence}`): AgentTurnRow {
  return { kind: "assistant", cursor: { sequence: String(sequence) }, sessionId: "session-one", runId: "run-one", turnId, startedAtMs: String(sequence), completedAtMs: String(sequence), text }
}

function props(rows: readonly AgentTurnRow[], overrides: Partial<ConversationPanelProps> = {}): WorkbenchPanelProps {
  return {
    title: "Transcript",
    selectedConversationId: "session-one",
    transcriptRows: rows,
    transcriptLoading: false,
    hasOlderTranscript: false,
    loadingOlderTranscript: false,
    messages: [],
    approvals: [],
    objective: "",
    attachments: [],
    onLoadOlderTranscript: () => {},
    onObjectiveChange: () => {},
    onRemoveAttachment: () => {},
    commands: createCommandDispatcher(new DesktopSettings(), () => ({ canStart: false, canCancel: false }), { openSettings() {}, openProject() {}, openDiagnostics() {}, openPlugins() {}, openBrowser() {}, focusPanel() {}, toggleTheme() {}, startRun() {}, cancelRun() {} }),
    onDecideApproval: () => {},
    closeBrowser: () => {},
    files: {
      entries: [],
      treeTruncated: false,
      treeLoading: false,
      selectedContent: undefined,
      contentLoading: false,
      draft: "",
      dirty: false,
      attached: false,
      attachmentEnabled: true,
      saving: false,
      selectEntry: () => {},
      setDraft: () => {},
      save: () => {},
      discard: () => {},
      toggleAttachment: () => {},
      openExternal: () => {},
      refreshTree: () => {},
      pdfPageIndex: 0,
      setPdfPageIndex: () => {},
    },
    artifacts: {
      artifacts: [],
      loading: false,
      selectArtifact: () => {},
      openImageArtifact: () => {},
      refresh: () => {},
      contentLoading: false,
      pdfPageIndex: 0,
      setPdfPageIndex: () => {},
    },
    terminal: {
      terminals: [],
      selectedTerminalId: undefined,
      selectedTerminal: undefined,
      viewport: undefined,
      snapshotTruncated: false,
      busy: false,
      error: undefined,
      canSpawn: false,
      setTerminalSurface: () => {},
      selectTerminal: () => {},
      spawn: async () => {},
      close: async () => {},
      sendInput: () => {},
      resize: () => {},
      refresh: async () => {},
    },
    git: {
      snapshot: undefined,
      visibleFiles: [],
      view: "unstaged",
      setView: () => {},
      selectedPaths: [],
      togglePath: () => {},
      patch: "",
      patchLoading: false,
      preparedRevert: undefined,
      cancelRevert: () => {},
      checkpoints: [],
      commitMessage: "",
      setCommitMessage: () => {},
      busy: false,
      loading: false,
      error: undefined,
      canStage: false,
      canUnstage: false,
      canCommit: false,
      stageSelected: () => {},
      unstageSelected: () => {},
      commit: () => {},
      prepareRevert: () => {},
      applyRevert: () => {},
      refresh: () => {},
    },
    codeHost: codeHostState(),
    threadWorkspace: {
      sessionId: "session-one",
      projection: { sessionId: "session-one", goal: "", plan: "", recap: "", notes: "", pins: [], workLog: [] },
      loading: false,
      drafts: { goal: "", plan: "", recap: "", notes: "" },
      dirty: { goal: false, plan: false, recap: false, notes: false },
      busy: false,
      setDraft: () => {},
      save: () => {},
      addPin: () => {},
      removePin: () => {},
      refresh: () => {},
    },
    openUrl: () => {},
    ...overrides,
  }
}

function conversation(propsValue: WorkbenchPanelProps) {
  return panelRegistry(propsValue).find((panel) => panel.id === "conversation")!.content
}

describe("M3 durable transcript", () => {
  it("keeps the approval Activity panel reachable until a reopen command exists", () => {
    expect(panelRegistry(props([])).find((panel) => panel.id === "activity")?.closable).toBe(false)
  })

  it("orders daemon pages by their numeric durable cursor", () => {
    expect(transcriptRows([
      { items: [assistant(11, "eleven"), user(2, "two")] },
      { items: [user(1, "one")] },
    ]).map((row) => row.cursor.sequence)).toEqual(["1", "2", "11"])
  })

  it("settles a completed live overlay only after that run has a durable assistant row", () => {
    expect(transcriptHasCommittedAssistant({ pages: [{ items: [user(1)] }], pageParams: [undefined] }, "run-1")).toBe(false)
    expect(transcriptHasCommittedAssistant({ pages: [{ items: [user(1), assistant(2, "done")] }], pageParams: [undefined] }, "run-one")).toBe(true)
  })

  it("renders durable user, markdown, tool, and pending rows with native elements", () => {
    const { render, renderer, unmount } = createTestRoot()
    const rows: AgentTurnRow[] = [
      user(1, "Please inspect this"),
      assistant(2, "## Result\n\nDone."),
      { kind: "toolCall", cursor: { sequence: "3" }, sessionId: "session-one", runId: "run-one", turnId: "turn-2", itemId: "tool-1", toolName: "shell", input: "{\"cmd\":\"pwd\"}", output: "/workspace", outcome: "completed", startedAtMs: "3", completedAtMs: "4" },
      { kind: "toolCall", cursor: { sequence: "4" }, sessionId: "session-one", runId: "run-one", turnId: "turn-2", itemId: "tool-2", toolName: "apply_patch", input: "{\"path\":\"README.md\"}", output: "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n", outcome: "completed", startedAtMs: "4", completedAtMs: "5" },
      { kind: "pendingState", cursor: { sequence: "5" }, sessionId: "session-one", runId: "run-one", occurredAtMs: "5", state: "waitingForApproval" },
    ]
    try {
      render(conversation(props(rows)))
      expect(renderer.findByType("virtual-list")).toHaveLength(1)
      expect(renderer.findByType("markdown")).toHaveLength(1)
      expect(renderer.findByType("code")).toHaveLength(3)
      expect(renderer.findByType("diff")).toHaveLength(1)
      const list = renderer.findByType("virtual-list")[0]!
      renderer.scrollToItem(list.id, 1)
      expect(renderer.getPaintedText()).toContain("Result")
      renderer.scrollToItem(list.id, 4)
      expect(renderer.getPaintedText()).toContain("WAITING FOR APPROVAL")
      renderer.scrollToItem(list.id, 0)
      expect(renderer.getPaintedText()).toContain("Please inspect this")
    } finally {
      unmount()
    }
  })

  it("mounts only the visible transcript window for 10,000 rows and suppresses a committed live turn", () => {
    const { render, renderer, unmount } = createTestRoot()
    const rows = Array.from({ length: 10_000 }, (_, index) => user(index + 1))
    rows.push(assistant(10_001, "durable answer", "turn-live"))
    try {
      render(conversation(props(rows, { messages: [{ id: "turn-live", text: "duplicate live answer" }] })))
      const list = renderer.findByType("virtual-list")[0]!
      expect(list.children.length).toBeLessThan(rows.length)
      expect(renderer.getAllText()).not.toContain("duplicate live answer")
      expect(renderer.getPaintedText()).toContain("durable answer")
    } finally {
      unmount()
    }
  })

  it("keeps transcript text selectable through the native renderer", () => {
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(conversation(props([user(1, "selectable request")])))
      const text = renderer.findByText("selectable request")!
      const bounds = renderer.getElementBounds(text.id)
      expect(bounds).not.toBeNull()
      const [x = 0, y = 0, width = 0, height = 0] = bounds ?? []
      const selected = renderer.dragSelect(x + 1, y + height / 2, x + width + 4, y + height / 2)
      expect(selected).toContain("selectable request")
    } finally {
      unmount()
    }
  })

  it("labels durable daemon image attachments as Image", () => {
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(conversation(props([user(1, "inspect this", [{ path: "pixel.png", revision: "rev", kind: "image", byteLen: "1" }])])))
      expect(renderer.getPaintedText()).toContain("Image · pixel.png")
    } finally {
      unmount()
    }
  })

  it("exposes transcript navigation and attachment removal as native keyboard controls", () => {
    const { render, renderer, unmount } = createTestRoot()
    let loadCount = 0
    const removed: string[] = []
    try {
      render(conversation(props([user(101)], {
        hasOlderTranscript: true,
        onLoadOlderTranscript: () => { loadCount += 1 },
        attachments: [{ path: "notes.md", expectedRevision: "revision-one" }],
        onRemoveAttachment: (path) => removed.push(path),
      })))
      const loadOlder = renderer.findByTestId("load-older-transcript")!
      renderer.nativeSimulateKeystrokes(loadOlder.id, "space")
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("remove-attachment-notes.md")!.id, "enter")
      expect(loadCount).toBe(1)
      expect(removed).toEqual(["notes.md"])
    } finally {
      unmount()
    }
  })

  it("keeps a manual scroll position when a live row arrives and resumes follow on demand", () => {
    const { render, renderer, unmount } = createTestRoot()
    const rows = Array.from({ length: 120 }, (_, index) => user(index + 1))
    try {
      render(conversation(props(rows)))
      const list = renderer.findByType("virtual-list")[0]!
      renderer.scrollToItem(list.id, 0)
      expect(renderer.findByTestId("jump-to-latest")).toBeDefined()

      render(conversation(props(rows, { messages: [{ id: "turn-live", text: "new live tail" }] })))
      expect(renderer.getPaintedText()).not.toContain("new live tail")

      const jump = renderer.findByTestId("jump-to-latest")!
      renderer.nativeSimulateKeystrokes(jump.id, "enter")
      expect(renderer.getPaintedText()).toContain("new live tail")
    } finally {
      unmount()
    }
  })
})
