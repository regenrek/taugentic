import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { QueryClient, QueryObserver } from "@tanstack/react-query"
import { describe, expect, it } from "bun:test"
import { act } from "react"

import type { AgentTurnRow, RunListEntry } from "@taugentic/desktop-protocol"

import { panelRegistry, type WorkbenchPanelProps } from "../src/features/workspace-layout/panels.js"
import { createCommandDispatcher } from "../src/features/commands/registry.js"
import { DesktopSettings } from "../src/platform/settings/desktop-settings.js"
import { conversationBranchRows, conversationBranchesQuery, invalidateConversationBranchesForLifecycleRecovery } from "../src/platform/daemon/conversation-branches-query.js"

const parentRow: AgentTurnRow = {
  kind: "assistant",
  cursor: { sequence: "42" },
  sessionId: "session-one",
  runId: "run-parent",
  turnId: "turn-parent",
  startedAtMs: "40",
  completedAtMs: "42",
  text: "Durable parent answer",
}

const child: RunListEntry = {
  id: "run-child",
  relationship: { kind: "fork", parentRunId: "run-parent", parentEventSeq: "42" },
  status: "running",
  harness: "native",
}

const grandchild: RunListEntry = {
  id: "run-grandchild",
  relationship: { kind: "fork", parentRunId: "run-child", parentEventSeq: "43" },
  status: "queued",
  harness: "native",
}

function conversation(props: Partial<WorkbenchPanelProps>) {
  const graph = props.branchGraph ?? { nodes: props.branches ?? [], edges: [], orphanRunIds: [], totalCount: (props.branches ?? []).length, omittedCount: 0, truncated: false, cycleBroken: false }
  return panelRegistry({
    title: "Parent conversation",
    selectedConversationId: "session-one",
    transcriptRows: [parentRow],
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
    files: {},
    artifacts: {},
    terminal: {},
    git: {},
    codeHost: {},
    openUrl: () => {},
    branchGraph: graph,
    ...props,
  } as WorkbenchPanelProps).find((panel) => panel.id === "conversation")!.content
}

function click(renderer: ReturnType<typeof createTestRoot>["renderer"], testId: string) {
  const element = renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id) ?? []
  renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

function renderedNode(tree: unknown, testId: string): { customProps?: Record<string, unknown> } | undefined {
  if (!tree || typeof tree !== "object") return undefined
  const node = tree as { testId?: string; children?: unknown[]; customProps?: Record<string, unknown> }
  if (node.testId === testId) return node
  return node.children?.map((child) => renderedNode(child, testId)).find(Boolean)
}

describe("M5 conversation branches", () => {
  it("renders one daemon-bound branch graph and Side Chat beside an unchanged parent, with isolated controls", () => {
    const root = createTestRoot()
    const opened: Array<[string, string]> = []
    const cancelled: string[] = []
    const closed: string[] = []
    const selectedBranches: string[] = []
    try {
      root.render(<div style={{ width: 1200, height: 760 }}>{conversation({ onOpenSideChat: (runId, sequence) => opened.push([runId, sequence]) })}</div>)
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("side-chat-42")!.id, "space")
      expect(opened).toEqual([["run-parent", "42"]])

      root.render(<div style={{ width: 1200, height: 760 }}>{conversation({ branches: [child, grandchild], sideChats: [child], onOpenSideChatPanel: (runId) => selectedBranches.push(runId), onCancelSideChat: (runId) => cancelled.push(runId), onCloseSideChat: (runId) => closed.push(runId) })}</div>)
      expect(root.renderer.findByTestId("conversation-branch-graph")).toBeDefined()
      const branchNode = root.renderer.findByTestId("branch-node-run-child")!
      expect(branchNode).toBeDefined()
      expect(root.renderer.findByTestId("branch-node-run-grandchild")).toBeDefined()
      root.renderer.nativeSimulateKeystrokes(branchNode.id, "enter")
      expect(selectedBranches).toEqual(["run-child"])
      expect(opened).toEqual([["run-parent", "42"]])
      expect(root.renderer.findByTestId("side-chat-panel-run-child")).toBeDefined()
      expect(root.renderer.findByTestId("side-chat-lineage-run-child")).toBeDefined()
      expect(root.renderer.getPaintedText()).toContain("Durable parent answer")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("cancel-side-chat-run-child")!.id, "space")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("close-side-chat-run-child")!.id, "enter")
      expect(cancelled).toEqual(["run-child"])
      expect(closed).toEqual(["run-child"])
    } finally {
      root.unmount()
    }
  })

  it("uses the lifecycle recovery owner to refetch one daemon child without creating a desktop branch", async () => {
    const calls: string[] = []
    let status: RunListEntry["status"] = child.status
    const runtime = {
      runLineageGraph: async (sessionId: string) => {
        calls.push(sessionId)
        return { nodes: [{ ...child, status }], edges: [], orphanRunIds: [], totalCount: 1, omittedCount: 0, truncated: false, cycleBroken: false }
      },
    }

    const query = conversationBranchesQuery(runtime as never, "session-one")
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    const observer = new QueryObserver(client, query)
    const unsubscribe = observer.subscribe(() => {})
    try {
      await observer.refetch()
      status = "cancelled"
      await invalidateConversationBranchesForLifecycleRecovery(client, "session-one")

      expect(calls).toEqual([
        "session-one",
        "session-one",
      ])
      expect(conversationBranchRows(observer.getCurrentResult().data)).toEqual([{ ...child, status: "cancelled" }])
      expect(conversationBranchRows(observer.getCurrentResult().data)).toHaveLength(1)
    } finally {
      unsubscribe()
      client.clear()
    }
  })

  it("submits a terminal Side Chat draft through the daemon continuation callback without a desktop transcript copy", () => {
    const root = createTestRoot()
    const continued: Array<[string, string]> = []
    const terminalChild = { ...child, status: "completed" as const }
    try {
      root.render(<div style={{ width: 1200, height: 760 }}>{conversation({
        sideChats: [terminalChild],
        onContinueSideChat: (runId, message) => continued.push([runId, message]),
      })}</div>)
      const input = root.renderer.findByTestId("continue-side-chat-input-run-child")!
      root.renderer.nativeSimulateInput(input.id, "continue only this branch")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("continue-side-chat-run-child")!.id, "space")
      expect(continued).toEqual([["run-child", "continue only this branch"]])
    } finally {
      root.unmount()
    }
  })

  it("keeps Side Chat Send unfocusable and inert when its continuation action is unavailable", () => {
    const root = createTestRoot()
    const continued: Array<[string, string]> = []
    const terminalChild = { ...child, status: "completed" as const }
    try {
      root.render(<div style={{ width: 1200, height: 760 }}>{conversation({
        sideChats: [terminalChild],
        onContinueSideChat: (runId, message) => continued.push([runId, message]),
      })}</div>)
      const input = root.renderer.findByTestId("continue-side-chat-input-run-child")!
      root.renderer.nativeSimulateInput(input.id, "must not submit")

      root.render(<div style={{ width: 1200, height: 760 }}>{conversation({ sideChats: [terminalChild] })}</div>)
      const send = root.renderer.findByTestId("continue-side-chat-run-child")!
      expect(renderedNode(root.renderer.toJSON(), "continue-side-chat-run-child")?.customProps?.tabIndex).toBe(-1)
      expect(JSON.parse(root.renderer.getAutomationTree())).toEqual(expect.objectContaining({ children: expect.any(Array) }))
      expect(root.renderer.getAutomationTree()).toContain('"testId":"continue-side-chat-run-child","accessibility":{"role":"button","name":"Send side chat message","disabled":true}')
      root.renderer.nativeSimulateKeystrokes(send.id, "space")
      click(root.renderer, "continue-side-chat-run-child")

      expect(continued).toEqual([])
    } finally {
      root.unmount()
    }
  })

  it("activates fresh-spawn, pin, and fresh-spawn cancellation through native keyboard controls", () => {
    const root = createTestRoot()
    const pins: Array<[string, string]> = []
    try {
      root.render(<div style={{ width: 1200, height: 760 }}>{conversation({
        onPinThreadWorkspace: (runId, cursor) => pins.push([runId, cursor]),
      })}</div>)
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("fresh-spawn-42")!.id, "enter")
      expect(root.renderer.findByTestId("fresh-spawn-composer-run-parent")).toBeDefined()
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("pin-thread-workspace-42")!.id, "enter")
      expect(pins).toEqual([["run-parent", "42"]])
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("dismiss-fresh-spawn-run-parent")!.id, "space")
      expect(root.renderer.findByTestId("fresh-spawn-composer-run-parent")).toBeUndefined()
    } finally {
      root.unmount()
    }
  })

  it("renders daemon-projected Fresh Spawn rows and keeps spawn and join interaction state ephemeral", async () => {
    const root = createTestRoot()
    const spawned: Array<[string, string]> = []
    const joined: Array<[string, string]> = []
    let joinCompletion: Promise<unknown> | undefined
    const fresh: RunListEntry = {
      id: "run-fresh-child",
      relationship: { kind: "freshSpawn", parentRunId: "run-parent" },
      status: "completed",
      harness: "codexAppServer",
    }
    try {
      root.render(<div style={{ width: 1200, height: 760 }}>{conversation({
        branches: [fresh],
        onSpawnFresh: (parentRunId, objective) => {
          spawned.push([parentRunId, objective])
          return Promise.resolve()
        },
        onJoinFresh: (parentRunId, childRunId) => {
          joined.push([parentRunId, childRunId])
          const completion = Promise.resolve({
            run: { id: childRunId, sessionId: "session-one", runtimeProfileId: "runtime-codex", objective: "fresh", status: "completed", harness: "codexAppServer", source: { kind: "freshSpawn", route: { runtimeProfileId: "runtime-codex", providerId: "codex", harness: "codexAppServer" }, parentRunId, workspaceScope: "workspaceWrite", cleanupPolicy: "deleteOnSuccess" }, executionContext: {} as never },
            result: { kind: "text", text: "finished" } as never,
            receipts: [{ id: "receipt-fresh", runId: childRunId, kind: "context", status: "accepted", summary: "joined" } as never],
            artifacts: [{ id: "artifact-fresh", runId: childRunId, kind: "patch", displayName: "fresh.patch" }],
          } as never)
          joinCompletion = completion
          return completion
        },
      })}</div>)
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("fresh-spawn-42")!.id, "enter")
      const objective = root.renderer.findByTestId("fresh-spawn-objective-run-parent")!
      root.renderer.nativeSimulateInput(objective.id, "independent investigation")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("spawn-fresh-run-run-parent")!.id, "space")
      expect(spawned).toEqual([["run-parent", "independent investigation"]])
      await act(async () => {
        root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("join-fresh-run-run-fresh-child")!.id, "enter")
        await joinCompletion!
      })
      root.renderer.flush()
      expect(joined).toEqual([["run-parent", "run-fresh-child"]])
      expect(root.renderer.findByTestId("fresh-join-status-run-fresh-child")).toBeDefined()
      expect(root.renderer.findByTestId("fresh-join-receipt-receipt-fresh")).toBeDefined()
      expect(root.renderer.findByTestId("fresh-join-artifact-artifact-fresh")).toBeDefined()
    } finally {
      root.unmount()
    }
  })
})
