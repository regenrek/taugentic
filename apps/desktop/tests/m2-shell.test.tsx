import { createActor } from "xstate"
import { describe, expect, it } from "bun:test"

import type { ApprovalRequest, DesktopDaemonLifecycleProjection, RunEventDelta, RunStatus } from "@taugentic/desktop-protocol"
import type { NativeDaemonBridge } from "@taugentic/desktop-daemon-native"

import { commandRegistry } from "../src/features/commands/registry.js"
import { workspaceShellMachine } from "../src/features/runtime/workspace-shell-machine.js"
import { projectConversations, sidebarReduce, type SidebarState } from "../src/features/sidebar/sidebar.js"
import { createDesktopRuntime } from "../src/platform/daemon/desktop-runtime.js"
import { desktopSettings } from "../src/platform/settings/desktop-settings.js"

function lifecycle(
  status: DesktopDaemonLifecycleProjection["status"],
  invalidated = status !== "ready",
): DesktopDaemonLifecycleProjection {
  return { status, invalidated, foreignRuntimeRestricted: false }
}

function assistantDelta(seq: string, delta: string, identity: { itemId?: string; turnId?: string } = { itemId: "item-one", turnId: "turn-one" }): RunEventDelta {
  return {
    seq,
    event: {
      agentStream: {
        runId: "run-one",
        turnId: identity.turnId,
        itemId: identity.itemId,
        fragmentSequence: seq,
        frame: { kind: "assistantMessageDelta", delta },
      },
    },
  }
}

function runStatusDelta(seq: string, status: RunStatus): RunEventDelta {
  return {
    seq,
    event: { run: { runId: "run-one", status, detail: status } },
  }
}

describe("M2 desktop shell ownership", () => {
  it("keeps one selector state and concrete workspace presentation", () => {
    const initial: SidebarState = { view: "spaces", filter: "", expandedSpaceIds: [] }
    const selected = sidebarReduce(initial, { type: "selectView", view: "agents" })
    expect(selected.view).toBe("agents")

    const workspaceId = "workspace-m2-test"
    expect(desktopSettings.presentation(workspaceId)).toBeUndefined()
    desktopSettings.savePresentation(workspaceId, {
      theme: "dark",
      layout: { kind: "tabs", id: "root", panels: ["conversation"], active: "conversation" },
    })
    expect(desktopSettings.presentation(workspaceId)?.layout.kind).toBe("tabs")
    expect(commandRegistry.map((command) => command.id)).toEqual(["focus-conversation", "focus-activity", "toggle-theme"])
  })

  it("stores project selection explicitly instead of deriving it from navigation order", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    expect(shell.getSnapshot().context.selectedProjectId).toBeUndefined()

    shell.send({ type: "PROJECT_SELECTED", projectId: "project-explicit" })

    expect(shell.getSnapshot().context.selectedProjectId).toBe("project-explicit")
  })

  it("clears conversation selection only when an explicit project row is chosen", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "SELECTED", sessionId: "conversation-a" })
    expect(shell.getSnapshot().context.sidebar.selectedConversationId).toBe("conversation-a")

    shell.send({ type: "PROJECT_SELECTED", projectId: "project-b" })

    expect(shell.getSnapshot().context.selectedProjectId).toBe("project-b")
    expect(shell.getSnapshot().context.sidebar.selectedConversationId).toBeUndefined()
  })

  it("projects conversations only under their daemon-owned selected project", () => {
    const snapshot = {
      spaces: [],
      projects: [],
      agents: [],
      conversations: [
        { sessionId: "conversation-a", title: "A", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: false },
        { sessionId: "conversation-b", title: "B", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-b" }, archived: false, pinned: false },
        { sessionId: "conversation-c", title: "C", status: "idle" as const, placement: { kind: "standalone" as const }, archived: false, pinned: false },
      ],
    }

    expect(projectConversations(snapshot, "project-b").map((conversation) => conversation.sessionId)).toEqual(["conversation-b"])
    expect(projectConversations(snapshot)).toEqual([])
  })

  it("delivers initial lifecycle before an early disconnected callback", async () => {
    let deliver: ((projectionJson: string) => void) | undefined
    let resolveInitial!: (projectionJson: string) => void
    const initial = lifecycle("ready", false)
    const bridge = {
      subscribeLifecycle(callback: (projectionJson: string) => void) {
        deliver = callback
        return new Promise<string>((resolve) => {
          resolveInitial = resolve
        })
      },
    } as unknown as NativeDaemonBridge
    const runtime = createDesktopRuntime(bridge)
    const shell = createActor(workspaceShellMachine).start()
    const received: DesktopDaemonLifecycleProjection[] = []
    const subscription = runtime.subscribeLifecycle((projection) => {
      received.push(projection)
      shell.send({ type: "LIFECYCLE", projection })
    })

    deliver?.(JSON.stringify(lifecycle("disconnected")))
    expect(received).toEqual([])
    resolveInitial(JSON.stringify(initial))

    expect(await subscription).toEqual(initial)
    expect(received).toEqual([initial, lifecycle("disconnected")])
    expect(shell.getSnapshot().context.phase).toBe("unavailable")
  })

  it("projects initial ready and history-gap lifecycle states without navigation owning availability", () => {
    const ready = createActor(workspaceShellMachine).start()
    ready.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    expect(ready.getSnapshot().context.phase).toBe("ready")
    expect(ready.getSnapshot().context.navigation).toBe("hydrating")

    const historyGap = createActor(workspaceShellMachine).start()
    historyGap.send({ type: "LIFECYCLE", projection: lifecycle("snapshotRehydrationRequired") })
    expect(historyGap.getSnapshot().context.phase).toBe("ready")
    expect(historyGap.getSnapshot().context.navigation).toBe("rehydrating")
  })

  it("keeps connectivity ready when navigation hydration fails", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "NAVIGATION_ERROR" })

    expect(shell.getSnapshot().context.phase).toBe("ready")
    expect(shell.getSnapshot().context.navigation).toBe("error")
    expect(shell.getSnapshot().context.navigationError).toBe("Navigation could not be refreshed. Your connection is still available.")
  })

  it("lets disconnected lifecycle produce unavailable without navigation owning availability", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "NAVIGATION_ERROR" })
    shell.send({ type: "LIFECYCLE", projection: lifecycle("disconnected") })
    shell.send({ type: "NAVIGATION_READY" })

    expect(shell.getSnapshot().context.phase).toBe("unavailable")
    expect(shell.getSnapshot().context.navigation).toBe("error")
  })

  it("maps only a rejected native start to unavailable and retains run errors", () => {
    const rejectedStart = createActor(workspaceShellMachine).start()
    rejectedStart.send({ type: "NATIVE_START_REJECTED" })

    expect(rejectedStart.getSnapshot().context.phase).toBe("unavailable")
    expect(rejectedStart.getSnapshot().context.error).toBeUndefined()

    const runFailure = createActor(workspaceShellMachine).start()
    runFailure.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    runFailure.send({ type: "ERROR", message: "The run could not be started." })

    expect(runFailure.getSnapshot().context.phase).toBe("ready")
    expect(runFailure.getSnapshot().context.error).toBe("The run could not be started.")
  })

  it("keeps a started run cancellable when only its event stream fails", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUN_STARTED", runId: "run-stream-failed" })
    shell.send({ type: "RUN_STREAM_ERROR", message: "The run stream failed." })

    expect(shell.getSnapshot().context.activeRun).toBe("run-stream-failed")
    expect(shell.getSnapshot().context.error).toBe("The run stream failed.")
  })

  it("projects concurrent auth login activity without mutating an active run", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUN_STARTED", runId: "run-auth-independent" })
    shell.send({ type: "AUTH_LOGIN_STARTED", authMethodId: "codex-chatgpt" })
    shell.send({ type: "AUTH_LOGIN_STARTED", authMethodId: "codex-chatgpt" })

    expect(shell.getSnapshot().context.pendingAuthMethodIds).toEqual(["codex-chatgpt", "codex-chatgpt"])

    shell.send({ type: "AUTH_LOGIN_FAILED", message: "The authentication profile could not be connected." })
    shell.send({ type: "AUTH_LOGIN_FINISHED", authMethodId: "codex-chatgpt" })

    expect(shell.getSnapshot().context.pendingAuthMethodIds).toEqual(["codex-chatgpt"])
    expect(shell.getSnapshot().context.activeRun).toBe("run-auth-independent")
    expect(shell.getSnapshot().context.error).toBe("The authentication profile could not be connected.")
  })

  it("merges rapid auth and model selection into one runtime draft", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime-codex-safe" } })
    shell.send({ type: "RUNTIME_DRAFT", draft: { authProfileId: "profile-codex" } })
    shell.send({ type: "RUNTIME_DRAFT", draft: { modelId: "gpt-5.6-sol" } })

    expect(shell.getSnapshot().context.runtimeDraft).toEqual({
      runtimeProfileId: "runtime-codex-safe",
      authProfileId: "profile-codex",
      modelId: "gpt-5.6-sol",
    })
  })

  it("clears provider-specific runtime choices when the runtime profile changes", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime-codex-safe" } })
    shell.send({ type: "RUNTIME_DRAFT", draft: { authProfileId: "profile-codex", modelId: "gpt-5.6-sol" } })
    shell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime-openai-safe" } })

    expect(shell.getSnapshot().context.runtimeDraft).toEqual({
      runtimeProfileId: "runtime-openai-safe",
    })
  })

  it("owns ordered assistant coalescing and terminal lifecycle in one run projection", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUN_STARTED", runId: "run-one" })

    expect(shell.getSnapshot().context.activeRun).toBe("run-one")
    expect(shell.getSnapshot().context.runStatus).toBe("running")

    shell.send({ type: "RUN_DELTAS", deltas: [assistantDelta("10", "world"), assistantDelta("9", "hello ")] })
    shell.send({ type: "RUN_DELTAS", deltas: [assistantDelta("10", "world"), assistantDelta("11", "!")] })

    expect(shell.getSnapshot().context.messages).toEqual([{ id: "item-one", text: "hello world!" }])

    shell.send({ type: "RUN_DELTAS", deltas: [runStatusDelta("12", "completed")] })

    expect(shell.getSnapshot().context.runStatus).toBe("completed")
    expect(shell.getSnapshot().context.activeRun).toBeUndefined()
    expect(shell.getSnapshot().context.messages).toEqual([{ id: "item-one", text: "hello world!" }])
  })

  it("owns pending approval projection with the active run lifecycle", () => {
    const shell = createActor(workspaceShellMachine).start()
    const approval: ApprovalRequest = {
      id: "approval-one",
      runId: "run-one",
      scope: "processExec",
      requestedAtMs: "1",
      expiresAtMs: "2",
      target: { kind: "processExec", command: "cargo check" },
      reason: "Run the requested verification",
    }
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUN_STARTED", runId: "run-one" })
    shell.send({ type: "APPROVALS_READY", approvals: [approval] })

    expect(shell.getSnapshot().context.approvals).toEqual([approval])

    shell.send({ type: "APPROVAL_DECIDED", approvalId: approval.id, runStatus: "running" })
    expect(shell.getSnapshot().context.approvals).toEqual([])
    expect(shell.getSnapshot().context.runStatus).toBe("running")

    shell.send({ type: "APPROVALS_READY", approvals: [approval] })
    shell.send({ type: "RUN_DELTAS", deltas: [runStatusDelta("2", "completed")] })
    expect(shell.getSnapshot().context.approvals).toEqual([])
  })

  it("projects a terminal approval result instead of leaving a phantom active run", () => {
    const shell = createActor(workspaceShellMachine).start()
    const approval: ApprovalRequest = {
      id: "approval-failed",
      runId: "run-failed",
      scope: "processExec",
      requestedAtMs: "1",
      expiresAtMs: "2",
      target: { kind: "processExec", command: "cargo check" },
      reason: "Run the requested verification",
    }
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUN_STARTED", runId: "run-failed" })
    shell.send({ type: "APPROVALS_READY", approvals: [approval] })

    shell.send({ type: "APPROVAL_DECIDED", approvalId: approval.id, runStatus: "failed" })

    expect(shell.getSnapshot().context.runStatus).toBe("failed")
    expect(shell.getSnapshot().context.activeRun).toBeUndefined()
    expect(shell.getSnapshot().context.approvals).toEqual([])
  })

  it("uses canonical turn and run identity when an assistant item id is absent", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUN_STARTED", runId: "run-one" })
    shell.send({
      type: "RUN_DELTAS",
      deltas: [
        assistantDelta("1", "turn ", { turnId: "turn-one" }),
        assistantDelta("2", "message", { turnId: "turn-one" }),
        assistantDelta("3", "run ", {}),
        assistantDelta("4", "message", {}),
      ],
    })

    expect(shell.getSnapshot().context.messages).toEqual([
      { id: "turn-one", text: "turn message" },
      { id: "run-one", text: "run message" },
    ])
  })

  it("keeps explicit close terminal against late lifecycle and navigation completion", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "CLOSED" })
    shell.send({ type: "LIFECYCLE", projection: lifecycle("disconnected") })
    shell.send({ type: "NAVIGATION_READY" })
    shell.send({ type: "SELECTED", sessionId: "late-session" })

    expect(shell.getSnapshot().context.phase).toBe("closed")
    expect(shell.getSnapshot().context.navigation).toBe("hydrating")
    expect(shell.getSnapshot().context.sidebar.selectedConversationId).toBeUndefined()
  })
})
