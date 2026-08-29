import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { QueryClientProvider } from "@tanstack/react-query"
import { describe, expect, it } from "bun:test"
import { useEffect, useRef } from "react"

import type { AgentTurnRow } from "@taugentic/desktop-protocol"

import { commandRegistry } from "../src/features/commands/registry.js"
import { ThreadWorkspacePanel } from "../src/features/thread-workspace/thread-workspace-panel.js"
import { useThreadWorkspace, type ThreadWorkspacePanelState } from "../src/features/thread-workspace/use-thread-workspace.js"
import { defaultWorkspaceLayout } from "../src/features/workspace-layout/layout-store.js"
import { panelRegistry, type WorkbenchPanelProps } from "../src/features/workspace-layout/panels.js"
import { createCommandDispatcher } from "../src/features/commands/registry.js"
import { DesktopSettings } from "../src/platform/settings/desktop-settings.js"
import { threadWorkspaceQuery, threadWorkspaceQueryKey, updateThreadWorkspace } from "../src/platform/daemon/thread-workspace-query.js"
import { desktopQueryClient } from "../src/platform/daemon/query-client.js"

const projection = {
  sessionId: "session-one",
  goal: "Ship the durable workspace",
  plan: "Keep the daemon authoritative",
  recap: "The projection is durable",
  notes: "Do not mirror domain state",
  pins: [{ runId: "run-one", cursor: { sequence: "42" } }],
  workLog: [
    { sequence: "2", occurredAtMs: "2", kind: "planSet" as const },
    { sequence: "11", occurredAtMs: "11", kind: "notesSet" as const },
  ],
}

function click(root: ReturnType<typeof createTestRoot>, testId: string) {
  const element = root.renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(element.id) ?? []
  root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

function keyActivate(root: ReturnType<typeof createTestRoot>, testId: string, key: "enter" | "space") {
  root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId(testId)!.id, key)
}

function state(overrides: Partial<ThreadWorkspacePanelState> = {}): ThreadWorkspacePanelState {
  return {
    sessionId: "session-one",
    projection,
    loading: false,
    drafts: { goal: projection.goal, plan: projection.plan, recap: projection.recap, notes: projection.notes },
    dirty: { goal: false, plan: false, recap: false, notes: false },
    busy: false,
    setDraft: () => {},
    save: () => {},
    addPin: () => {},
    removePin: () => {},
    refresh: () => {},
    ...overrides,
  }
}

function workbenchProps(overrides: Partial<WorkbenchPanelProps> = {}): WorkbenchPanelProps {
  return {
    title: "Conversation",
    selectedConversationId: "session-one",
    transcriptRows: [],
    transcriptLoading: false,
    hasOlderTranscript: false,
    loadingOlderTranscript: false,
    onLoadOlderTranscript: () => {},
    messages: [],
    approvals: [],
    objective: "",
    attachments: [],
    onObjectiveChange: () => {},
    onRemoveAttachment: () => {},
    commands: createCommandDispatcher(new DesktopSettings(), () => ({ canStart: false, canCancel: false }), { openSettings() {}, focusPanel() {}, toggleTheme() {}, startRun() {}, cancelRun() {} }),
    onDecideApproval: () => {},
    files: {} as WorkbenchPanelProps["files"],
    artifacts: {} as WorkbenchPanelProps["artifacts"],
    terminal: {} as WorkbenchPanelProps["terminal"],
    git: {} as WorkbenchPanelProps["git"],
    codeHost: {} as WorkbenchPanelProps["codeHost"],
    threadWorkspace: state(),
    openUrl: () => {},
    ...overrides,
  }
}

describe("M3 thread workspace", () => {
  it("renders daemon projection, native inputs, ordered log, and deterministic panel placement", () => {
    const root = createTestRoot()
    try {
      root.render(<div style={{ width: 900, height: 760 }}><ThreadWorkspacePanel workspace={state()} /></div>)
      expect(root.renderer.findByTestId("thread-workspace-goal")).toBeDefined()
      expect(root.renderer.findByTestId("thread-workspace-plan")).toBeDefined()
      expect(root.renderer.findByTestId("thread-workspace-recap")).toBeDefined()
      expect(root.renderer.findByTestId("thread-workspace-notes")).toBeDefined()
      expect(root.renderer.findByTestId("thread-workspace-pin-run-one-42")).toBeDefined()
      expect(root.renderer.findByTestId("thread-work-log-2")).toBeDefined()
      expect(root.renderer.findByTestId("thread-work-log-11")).toBeDefined()
      expect(root.renderer.getPaintedText()).toContain("Ship the durable workspace")
      expect(defaultWorkspaceLayout.kind).toBe("split")
      expect(JSON.stringify(defaultWorkspaceLayout)).toContain("thread-workspace")
      expect(panelRegistry(workbenchProps()).find((panel) => panel.id === "thread-workspace")?.closable).toBe(false)
      expect(commandRegistry.find((command) => command.id === "focus-thread-workspace")?.panelId).toBe("thread-workspace")
    } finally {
      root.unmount()
    }
  })

  it("submits the four editable fields, refresh, and removal only as panel intents", () => {
    const root = createTestRoot()
    const saves: string[] = []
    const removed: string[] = []
    let refreshed = 0
    try {
      root.render(<div style={{ width: 900, height: 760 }}><ThreadWorkspacePanel workspace={state({ save: (field) => saves.push(field), removePin: (cursor) => removed.push(cursor), refresh: () => { refreshed += 1 } })} /></div>)
      click(root, "save-thread-workspace-goal")
      click(root, "save-thread-workspace-plan")
      click(root, "save-thread-workspace-recap")
      click(root, "save-thread-workspace-notes")
      click(root, "remove-thread-workspace-pin-run-one-42")
      click(root, "refresh-thread-workspace")
      expect(saves).toEqual(["goal", "plan", "recap", "notes"])
      expect(removed).toEqual(["42"])
      expect(refreshed).toBe(1)
    } finally {
      root.unmount()
    }
  })

  it("activates save, refresh, and remove controls through native keyboard events", () => {
    const root = createTestRoot()
    const saves: string[] = []
    const removed: string[] = []
    let refreshed = 0
    try {
      root.render(<div style={{ width: 900, height: 760 }}><ThreadWorkspacePanel workspace={state({ save: (field) => saves.push(field), removePin: (cursor) => removed.push(cursor), refresh: () => { refreshed += 1 } })} /></div>)
      keyActivate(root, "save-thread-workspace-goal", "enter")
      keyActivate(root, "save-thread-workspace-plan", "space")
      keyActivate(root, "refresh-thread-workspace", "space")
      keyActivate(root, "remove-thread-workspace-pin-run-one-42", "enter")
      expect(saves).toEqual(["goal", "plan"])
      expect(removed).toEqual(["42"])
      expect(refreshed).toBe(1)
    } finally {
      root.unmount()
    }
  })

  it("pins an exact durable transcript boundary without a transcript copy", () => {
    const root = createTestRoot()
    const pins: Array<[string, string]> = []
    const row: AgentTurnRow = { kind: "assistant", cursor: { sequence: "42" }, sessionId: "session-one", runId: "run-one", turnId: "turn-one", startedAtMs: "42", completedAtMs: "42", text: "Durable answer" }
    try {
      const conversation = panelRegistry(workbenchProps({ transcriptRows: [row], onPinThreadWorkspace: (runId, cursor) => pins.push([runId, cursor]) })).find((panel) => panel.id === "conversation")!.content
      root.render(<div style={{ width: 900, height: 760 }}>{conversation}</div>)
      keyActivate(root, "pin-thread-workspace-42", "enter")
      expect(pins).toEqual([["run-one", "42"]])
    } finally {
      root.unmount()
    }
  })

  it("uses the one session-scoped query owner for rereads and exact daemon mutations", async () => {
    const calls: string[] = []
    const runtime = {
      bridge: {
        threadWorkspace: async () => {
          calls.push("get")
          return JSON.stringify(projection)
        },
        updateThreadWorkspace: async (command: string) => {
          calls.push(command)
          return JSON.stringify(projection)
        },
      },
    } as never
    const query = threadWorkspaceQuery(runtime, "session-one")
    expect(threadWorkspaceQueryKey("session-one")).toEqual(["daemon", "thread-workspace", "session-one"])
    expect(await query.queryFn!({ client: {} as never, queryKey: query.queryKey, signal: new AbortController().signal, meta: undefined })).toEqual(projection)
    expect(await query.queryFn!({ client: {} as never, queryKey: query.queryKey, signal: new AbortController().signal, meta: undefined })).toEqual(projection)
    await updateThreadWorkspace(runtime, { kind: "goalSet", value: "new goal" })
    await updateThreadWorkspace(runtime, { kind: "planSet", value: "new plan" })
    await updateThreadWorkspace(runtime, { kind: "recapSet", value: "new recap" })
    await updateThreadWorkspace(runtime, { kind: "notesSet", value: "new notes" })
    expect(calls.slice(0, 2)).toEqual(["get", "get"])
    expect(calls.slice(2).map((call) => JSON.parse(call))).toEqual([
      { mutation: { kind: "goalSet", value: "new goal" } },
      { mutation: { kind: "planSet", value: "new plan" } },
      { mutation: { kind: "recapSet", value: "new recap" } },
      { mutation: { kind: "notesSet", value: "new notes" } },
    ])
  })

  it("clears only a successfully saved draft, leaving projection data daemon-owned", async () => {
    desktopQueryClient.clear()
    desktopQueryClient.setQueryData(threadWorkspaceQueryKey("session-one"), projection)
    const updates: string[] = []
    let settled!: () => void
    const settledPromise = new Promise<void>((resolve) => { settled = resolve })
    const runtime = {
      bridge: {
        threadWorkspace: async () => JSON.stringify(projection),
        updateThreadWorkspace: async (command: string) => {
          updates.push(command)
          return JSON.stringify({ ...projection, goal: "Saved only this goal" })
        },
      },
    } as never
    function Harness() {
      const workspace = useThreadWorkspace({ runtime, sessionId: "session-one", enabled: true })
      const wasDirty = useRef(false)
      useEffect(() => {
        if (workspace.dirty.goal) wasDirty.current = true
        if (wasDirty.current && !workspace.dirty.goal) settled()
      }, [workspace.dirty.goal])
      return <div style={{ width: 900, height: 760 }}>
        <div testId="make-goal-dirty" tabIndex={0} onClick={() => workspace.setDraft("goal", "Saved only this goal")}><text>Draft goal</text></div>
        <text testId="goal-dirty">{String(workspace.dirty.goal)}</text>
        <text testId="plan-dirty">{String(workspace.dirty.plan)}</text>
        <ThreadWorkspacePanel workspace={workspace} />
      </div>
    }
    const root = createTestRoot()
    try {
      root.render(<QueryClientProvider client={desktopQueryClient}><Harness /></QueryClientProvider>)
      click(root, "make-goal-dirty")
      expect(root.renderer.getPaintedText()).toContain("true")
      click(root, "save-thread-workspace-goal")
      await settledPromise
      root.renderer.flush()
      expect(updates).toEqual([JSON.stringify({ mutation: { kind: "goalSet", value: "Saved only this goal" } })])
      expect(root.renderer.getPaintedText().filter((text) => text === "true")).toEqual([])
      expect(root.renderer.getPaintedText().filter((text) => text === "false")).toHaveLength(2)
    } finally {
      root.unmount()
      desktopQueryClient.clear()
    }
  })
})
