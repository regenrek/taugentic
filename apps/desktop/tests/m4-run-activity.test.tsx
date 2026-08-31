import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import { RunActivityPanel } from "../src/features/run-activity/run-activity-panel.js"
import { getNativeRunsPage, nativeRunHistoryPageSize } from "../src/platform/daemon/run-activity-query.js"
import { requestSwitchRouteAndResume } from "../src/features/run-activity/use-run-activity.js"
import type { ReturnTypeUseRunActivity } from "../src/features/run-activity/types.js"

function click(root: ReturnType<typeof createTestRoot>, testId: string) {
  const element = root.renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(element.id) ?? []
  root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

function scrollPanelAndClick(root: ReturnType<typeof createTestRoot>, testId: string) {
  const panel = root.renderer.findByTestId("run-activity-content")!
  const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(panel.id) ?? []
  root.renderer.nativeSimulateScrollWheel(x + width / 2, y + height / 2, 0, -1000)
  click(root, testId)
}

function renderedNode(tree: unknown, testId: string): { customProps?: Record<string, unknown>; style?: Record<string, unknown> } | undefined {
  if (!tree || typeof tree !== "object") return undefined
  const node = tree as { testId?: string; children?: unknown[]; customProps?: Record<string, unknown>; style?: Record<string, unknown> }
  if (node.testId === testId) return node
  return node.children?.map((child) => renderedNode(child, testId)).find(Boolean)
}

describe("M4 run activity", () => {
  it("requests native run history with the canonical limit and forwards opaque cursors", async () => {
    const requests: Array<{ sessionId: string; request: { limit: number; cursor?: string } }> = []
    const runtime = { bridge: { listNativeRuns: async (sessionId: string, request: string) => {
      requests.push({ sessionId, request: JSON.parse(request) })
      return JSON.stringify({ runs: [], nextCursor: "opaque-next-cursor" })
    } } } as never

    await getNativeRunsPage(runtime, "session-one" as never)
    await getNativeRunsPage(runtime, "session-one" as never, "opaque-next-cursor")

    expect(requests).toEqual([
      { sessionId: "session-one", request: { limit: nativeRunHistoryPageSize } },
      { sessionId: "session-one", request: { limit: nativeRunHistoryPageSize, cursor: "opaque-next-cursor" } },
    ])
  })

  it("sends one complete explicit route-replacement request and leaves invalid input untouched", async () => {
    const requests: unknown[] = []
    const runtime = {
      switchRouteAndResume: async (request: unknown) => { requests.push(request) },
    } as never
    const selection = {
      runtimeProfileId: "runtime-openai-safe",
      authProfileId: "profile-replacement",
      modelId: "gpt-5.6-sol",
    } as never

    expect(await requestSwitchRouteAndResume(runtime, {
      sessionId: "session-one" as never,
      parentRunId: "run-parent" as never,
      exhausted: true,
      replacementSelection: selection,
    })).toBe(true)
    expect(requests).toEqual([{
      sessionId: "session-one",
      parentRunId: "run-parent",
      selection,
    }])

    expect(await requestSwitchRouteAndResume(runtime, {
      sessionId: "session-one" as never,
      parentRunId: "run-parent" as never,
      exhausted: false,
      replacementSelection: selection,
    })).toBe(false)
    expect(requests).toHaveLength(1)
  })

  it("renders durable daemon projections and sends the visible run-activity intents", () => {
    const selected: string[] = []
    const approvals: string[] = []
    const cancelled: string[] = []
    const artifacts: string[] = []
    const switches: string[] = []
    const olderRunRequests: string[] = []
    const olderActivityRequests: string[] = []
    const longProvider = `provider-${"x".repeat(160)}`
    const boundedProvider = `${longProvider.slice(0, 117)}...`
    const state = {
      runs: [
        { id: "run-one", relationship: { kind: "root" }, harness: "native", status: "waitingForApproval", objectivePreview: "Verify the work" },
        { id: "run-child", relationship: { kind: "fork", parentRunId: "run-one" }, harness: "native", status: "running", objectivePreview: "Inspect the result" },
      ],
      selectedRunId: "run-one",
      selectRun: (runId: string) => selected.push(runId),
      detail: { summary: { id: "run-one", runtimeProfileId: "profile-one", objective: "Verify the work", status: "waitingForApproval" }, executionContext: {}, authProfileExhaustion: "creditsExhausted" },
      timeline: { sessionId: "session-one", rootRunId: "run-one", runs: [{ runId: "run-one", depth: 0, status: "waitingForApproval" }, { runId: "run-child", parentRunId: "run-one", depth: 1, status: "running" }], events: [{ seq: "6", occurredAtMs: "6", runId: "run-one", kind: "runStatus", label: "The selected account has exhausted its credits.", status: "failed", payload: { kind: "run", detail: "The selected account has exhausted its credits.", auth_profile_exhaustion: "creditsExhausted" } }, { seq: "7", occurredAtMs: "7", runId: "run-one", kind: "artifact", label: "Created report", payload: { kind: "artifact", artifactId: "artifact-one", artifactKind: "File" } }] },
      replay: [{ seq: "6", event: { run: { kind: "status", payload: { runId: "run-one", status: "waitingForApproval" } } } }, { seq: "7", event: { run: { kind: "status", payload: { runId: "run-one", status: "waitingForApproval" } } } }, { seq: "8", event: { tokenUsageRecorded: { runId: "run-one", promptTokens: "1", completionTokens: "2", model: "model-one", provider: longProvider, recordedAtMs: "8" } } }],
      activity: [{ cursor: { sequence: "9" }, occurredAtMs: "9", event: { approval: { kind: "requested", approval: { id: "approval-one", runId: "run-one", scope: "processExec", requestedAtMs: "1", target: { kind: "processExec" }, reason: "Verify" } } } }, { cursor: { sequence: "8" }, occurredAtMs: "8", event: { run: { kind: "status", payload: { runId: "run-one", status: "failed", reason: "The selected account has exhausted its credits.", authProfileExhaustion: "creditsExhausted" } } } }],
      approvals: [{ id: "approval-one", runId: "run-one", scope: "processExec", requestedAtMs: "1", target: { kind: "processExec" }, reason: "Verify" }],
      loading: false,
      hasOlderRuns: true,
      loadingOlderRuns: false,
      loadOlderRuns: () => { olderRunRequests.push("older") },
      hasOlderActivity: true,
      loadingOlderActivity: false,
      loadOlderActivity: () => { olderActivityRequests.push("older") },
      error: undefined,
      refresh: () => {},
      decide: async (id: string, decision: string) => { approvals.push(`${id}:${decision}`) },
      cancel: async (id: string) => { cancelled.push(id) },
      openArtifact: (id: string) => artifacts.push(id),
      switchEligible: true,
      switchRouteAndResume: async () => { switches.push("switch") },
    } as unknown as ReturnTypeUseRunActivity
    const root = createTestRoot()
    try {
      root.render(<div style={{ width: 800, height: 760 }}><RunActivityPanel activity={state} /></div>)
      expect(root.renderer.findByTestId("run-run-one")).toBeDefined()
      expect(root.renderer.findByTestId("run-run-child")).toBeDefined()
      expect(root.renderer.findByType("virtual-list")).toHaveLength(1)
      expect(root.renderer.findByTestId("run-detail")).toBeDefined()
      expect(root.renderer.findByTestId("run-auth-profile-exhaustion")).toBeDefined()
      expect(root.renderer.findByTestId("timeline-exhaustion-6")).toBeDefined()
      expect(root.renderer.findByTestId("timeline-run-run-one")).toBeDefined()
      expect(root.renderer.findByTestId("timeline-run-run-child")).toBeDefined()
      expect(root.renderer.findByTestId("timeline-7")).toBeDefined()
      expect(root.renderer.findByTestId("activity-9")).toBeDefined()
      expect(root.renderer.findByTestId("activity-8")).toBeDefined()
      expect(root.renderer.findByTestId("replay-6")).toBeDefined()
      expect(root.renderer.findByTestId("replay-7")).toBeDefined()
      expect(root.renderer.findByTestId("replay-8")).toBeDefined()
      expect(root.renderer.findByTestId("load-older-activity")).toBeDefined()
      expect(root.renderer.findByTestId("load-older-runs")).toBeDefined()
      expect(root.renderer.getAllText()).toContain("Account creditsExhausted")
      expect(root.renderer.getAllText()).toContain("Run status changed")
      expect(root.renderer.getAllText()).toContain("waitingForApproval")
      expect(root.renderer.getAllText()).toContain(boundedProvider)
      expect(root.renderer.getAllText()).not.toContain(longProvider)
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("cancel-selected-run")!.id, "space")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("approve-approval-one")!.id, "enter")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("open-artifact-artifact-one")!.id, "space")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("switch-route-and-resume")!.id, "enter")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("run-run-child")!.id, "space")
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("load-older-runs")!.id, "enter")
      const activityPanel = root.renderer.findByTestId("run-activity-content")!
      const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(activityPanel.id) ?? []
      root.renderer.nativeSimulateScrollWheel(x + width / 2, y + height / 2, 0, -1000)
      root.renderer.nativeSimulateKeystrokes(root.renderer.findByTestId("load-older-activity")!.id, "enter")
      expect(selected).toEqual(["run-child"])
      expect(olderRunRequests).toEqual(["older"])
      expect(olderActivityRequests).toEqual(["older"])
      expect(approvals).toEqual(["approval-one:approved"])
      expect(cancelled).toEqual(["run-one"])
      expect(artifacts).toEqual(["artifact-one"])
      expect(switches).toEqual(["switch"])

      state.detail!.summary.status = "cancelled"
      root.render(<div style={{ width: 800, height: 760 }}><RunActivityPanel activity={state} /></div>)
      expect(root.renderer.findByTestId("cancel-selected-run")).toBeUndefined()
      state.switchEligible = false
      root.render(<div style={{ width: 800, height: 760 }}><RunActivityPanel activity={state} /></div>)
      expect(root.renderer.findByTestId("switch-route-and-resume")).toBeUndefined()
    } finally {
      root.unmount()
    }
  })

  it("keeps Load older activity unfocusable and inert while its one availability fact is unavailable", () => {
    const olderActivityRequests: string[] = []
    const state = {
      runs: [], selectedRunId: undefined, selectRun: () => {}, detail: undefined, timeline: undefined, replay: [], activity: [], approvals: [], loading: false,
      hasOlderRuns: false, loadingOlderRuns: false, loadOlderRuns: () => {},
      hasOlderActivity: true, loadingOlderActivity: true, loadOlderActivity: () => { olderActivityRequests.push("older") }, error: undefined, refresh: () => {}, decide: async () => {}, cancel: async () => {}, openArtifact: () => {}, switchEligible: false, switchRouteAndResume: async () => {},
    } as unknown as ReturnTypeUseRunActivity
    const root = createTestRoot()
    try {
      root.render(<div style={{ width: 800, height: 760 }}><RunActivityPanel activity={state} /></div>)
      const loadOlder = root.renderer.findByTestId("load-older-activity")!
      const rendered = renderedNode(root.renderer.toJSON(), "load-older-activity")
      expect(rendered?.customProps?.tabIndex).toBe(-1)
      expect(rendered?.style).toMatchObject({ cursor: "default", backgroundColor: "#151922" })
      expect(root.renderer.getAutomationTree()).toContain('"testId":"load-older-activity","accessibility":{"role":"button","name":"Load older activity","disabled":true}')
      root.renderer.nativeSimulateKeystrokes(loadOlder.id, "enter")
      scrollPanelAndClick(root, "load-older-activity")
      expect(olderActivityRequests).toEqual([])

      state.loadingOlderActivity = false
      root.render(<div style={{ width: 800, height: 760 }}><RunActivityPanel activity={state} /></div>)
      const enabled = root.renderer.findByTestId("load-older-activity")!
      expect(renderedNode(root.renderer.toJSON(), "load-older-activity")?.customProps?.tabIndex).toBe(0)
      expect(renderedNode(root.renderer.toJSON(), "load-older-activity")?.style).toMatchObject({ cursor: "pointer", backgroundColor: "#143628" })
      expect(root.renderer.getAutomationTree()).toContain('"testId":"load-older-activity","accessibility":{"role":"button","name":"Load older activity","disabled":false}')
      root.renderer.nativeSimulateKeystrokes(enabled.id, "space")
      expect(olderActivityRequests).toEqual(["older"])
    } finally {
      root.unmount()
    }
  })
})
