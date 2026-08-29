import { createActor } from "xstate"
import { describe, expect, it } from "bun:test"
import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { QueryClientProvider } from "@tanstack/react-query"

import type { AgentRuntimeSnapshot, DesktopDaemonLifecycleProjection, RunEventDelta, RunStatus } from "@taugentic/desktop-protocol"
import type { NativeDaemonBridge } from "@taugentic/desktop-daemon-native"

import { commandRegistry, createCommandDispatcher } from "../src/features/commands/registry.js"
import { RuntimeRoutePicker } from "../src/features/auth-profiles/auth-profiles.js"
import { ProjectTrustConfirmation } from "../src/app/project-trust-confirmation.js"
import { App, Workbench, workbenchSelection } from "../src/app/App.js"
import { workspaceShellMachine } from "../src/features/runtime/workspace-shell-machine.js"
import { archiveConversation, closeTemporaryConversation, createProjectConversation, createSpace, createStandaloneConversation, createTemporaryConversation, desktopRuntime, openProject, selectConversation, setConversationPinned, setProjectSpace, startSelectedRun, triggerWorkItem, workspaceShell } from "../src/features/runtime/workspace-shell.js"
import { defaultWorkspaceLayout, workspacePresentation } from "../src/features/workspace-layout/layout-store.js"
import { archivedProjectConversations, projectConversations, sidebarReduce, Sidebar, standaloneConversations, temporaryConversations, type SidebarState } from "../src/features/sidebar/sidebar.js"
import { createDesktopRuntime } from "../src/platform/daemon/desktop-runtime.js"
import { navigationQuery, navigationQueryKey } from "../src/platform/daemon/navigation-query.js"
import { desktopQueryClient } from "../src/platform/daemon/query-client.js"
import { runActivityQueryRoot } from "../src/platform/daemon/run-activity-query.js"
import { scheduledWorkQueryKey } from "../src/platform/daemon/scheduled-work-query.js"
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
    event: { run: { kind: "status", payload: { runId: "run-one", status, authProfileExhaustion: null } } },
  }
}

describe("M2 run lifecycle", () => {
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

  it("delivers a terminal attach snapshot before buffered live output", async () => {
    let deliver: ((eventJson: string) => void) | undefined
    let resolveInitial!: (initialJson: string) => void
    const bridge = {
      subscribeTerminalEvents(_terminalId: string, callback: (eventJson: string) => void) {
        deliver = callback
        return new Promise<string>((resolve) => {
          resolveInitial = resolve
        })
      },
      releaseTerminalEventSubscription() {},
    } as unknown as NativeDaemonBridge
    const runtime = createDesktopRuntime(bridge)
    const received: string[] = []
    const subscription = runtime.subscribeTerminal("terminal-one", {
      attached: (initial) => received.push(`snapshot:${initial.snapshotBase64}`),
      event: (event) => received.push(`event:${event.event.kind}`),
      failed: (message) => received.push(`failed:${message}`),
    })

    deliver?.(JSON.stringify({
      terminalId: "terminal-one",
      event: { kind: "output", sequence: "2", dataBase64: "bGl2ZQ==" },
    }))
    expect(received).toEqual([])
    resolveInitial(JSON.stringify({
      terminal: { id: "terminal-one", projectId: "project-one", workspaceId: "workspace-one", status: "running", rows: 24, cols: 80 },
      snapshotBase64: "c25hcHNob3Q=",
      snapshotTruncated: false,
      latestSequence: "1",
    }))

    await subscription
    expect(received).toEqual(["snapshot:c25hcHNob3Q=", "event:output"])
  })

  it("delivers a terminal stream failure after its attach snapshot without throwing from the native callback", async () => {
    let deliver: ((eventJson: string) => void) | undefined
    let resolveInitial!: (initialJson: string) => void
    const bridge = {
      subscribeTerminalEvents(_terminalId: string, callback: (eventJson: string) => void) {
        deliver = callback
        return new Promise<string>((resolve) => {
          resolveInitial = resolve
        })
      },
      releaseTerminalEventSubscription() {},
    } as unknown as NativeDaemonBridge
    const runtime = createDesktopRuntime(bridge)
    const received: string[] = []
    const subscription = runtime.subscribeTerminal("terminal-one", {
      attached: () => received.push("snapshot"),
      event: () => received.push("event"),
      failed: (message) => received.push(message),
    })

    expect(() => deliver?.("native daemon event stream closed")).not.toThrow()
    expect(received).toEqual([])
    resolveInitial(JSON.stringify({
      terminal: { id: "terminal-one", projectId: "project-one", workspaceId: "workspace-one", status: "running", rows: 24, cols: 80 },
      snapshotBase64: "",
      snapshotTruncated: false,
      latestSequence: "0",
    }))

    await subscription
    expect(received).toEqual(["snapshot", "The terminal event stream ended unexpectedly."])
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
    shell.send({ type: "RUN_STREAM_ERROR", runId: "run-stream-failed", message: "The run stream failed." })

    expect(shell.getSnapshot().context.activeRun).toBe("run-stream-failed")
    expect(shell.getSnapshot().context.error).toBe("The run stream failed.")
  })

  it("settles cancellation without letting its closing stream overwrite the run state", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUN_STARTED", runId: "run-cancelled" })
    shell.send({ type: "RUN_STREAM_ERROR", runId: "run-cancelled", message: "The stream closed." })
    shell.send({ type: "RUN_CANCELLED", runId: "run-cancelled" })
    shell.send({ type: "RUN_STREAM_ERROR", runId: "run-cancelled", message: "A late stream error." })

    expect(shell.getSnapshot().context.activeRun).toBeUndefined()
    expect(shell.getSnapshot().context.runStatus).toBe("cancelled")
    expect(shell.getSnapshot().context.error).toBeUndefined()
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

    expect(shell.getSnapshot().context.pendingSelection).toEqual({
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

    expect(shell.getSnapshot().context.pendingSelection).toEqual({
      runtimeProfileId: "runtime-openai-safe",
    })
  })

  it("owns ordered assistant coalescing and terminal lifecycle in one run projection", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUN_STARTED", runId: "run-one" })

    expect(shell.getSnapshot().context.activeRun).toBe("run-one")
    expect(shell.getSnapshot().context.runStatus).toBe("running")

    shell.send({ type: "RUN_DELTAS", runId: "run-one", deltas: [assistantDelta("10", "world"), assistantDelta("9", "hello ")] })
    shell.send({ type: "RUN_DELTAS", runId: "run-one", deltas: [assistantDelta("10", "world"), assistantDelta("11", "!")] })

    expect(shell.getSnapshot().context.messages).toEqual([{ id: "turn-one", text: "hello world!" }])

    shell.send({ type: "RUN_DELTAS", runId: "run-one", deltas: [runStatusDelta("12", "completed")] })

    expect(shell.getSnapshot().context.runStatus).toBe("completed")
    expect(shell.getSnapshot().context.activeRun).toBeUndefined()
    expect(shell.getSnapshot().context.messages).toEqual([{ id: "turn-one", text: "hello world!" }])

    shell.send({ type: "TRANSCRIPT_COMMITTED", runId: "run-one" })
    expect(shell.getSnapshot().context.messages).toEqual([])
    expect(shell.getSnapshot().context.runDeltas).toEqual([])

    shell.send({ type: "RUN_DELTAS", runId: "run-one", deltas: [assistantDelta("13", "late")] })
    expect(shell.getSnapshot().context.messages).toEqual([])
  })

  it("uses canonical turn and run identity when an assistant item id is absent", () => {
    const shell = createActor(workspaceShellMachine).start()
    shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
    shell.send({ type: "RUN_STARTED", runId: "run-one" })
    shell.send({
      type: "RUN_DELTAS",
      runId: "run-one",
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

  it("forwards the exact image draft through the selected route and preserves it on a safe bridge rejection", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      startRun(commandJson: string): Promise<string>
      subscribeRunEvents(sessionId: string, runId: string, callback: (eventJson: string) => void): Promise<string>
    }
    const originalStartRun = bridge.startRun
    const originalSubscribeRunEvents = bridge.subscribeRunEvents
    const commands: unknown[] = []
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "session-image" })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime-image" } })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { authProfileId: "auth-image", modelId: "model-image" } })
      workspaceShell.send({ type: "SET_OBJECTIVE", objective: "Describe this image" })
      workspaceShell.send({ type: "TOGGLE_ATTACHMENT", attachment: { path: "pixel.png", expectedRevision: "revision-image" } })
      bridge.startRun = async (commandJson) => {
        commands.push(JSON.parse(commandJson))
        return JSON.stringify({ id: "run-image" })
      }
      bridge.subscribeRunEvents = async () => JSON.stringify({ events: [] })

      await startSelectedRun()

      expect(commands).toEqual([{
        objective: "Describe this image",
        selection: { runtimeProfileId: "runtime-image", authProfileId: "auth-image", modelId: "model-image" },
        attachments: [{ path: "pixel.png", expectedRevision: "revision-image" }],
      }])
      expect(workspaceShell.getSnapshot().context.activeRun).toBe("run-image")
      expect(workspaceShell.getSnapshot().context.objective).toBe("")
      expect(workspaceShell.getSnapshot().context.attachments).toEqual([])

      workspaceShell.send({ type: "RUN_CANCELLED", runId: "run-image" })
      workspaceShell.send({ type: "SET_OBJECTIVE", objective: "Retry the image" })
      workspaceShell.send({ type: "TOGGLE_ATTACHMENT", attachment: { path: "pixel.png", expectedRevision: "revision-image" } })
      bridge.startRun = async () => { throw new Error("daemon rejected image") }

      await startSelectedRun()

      expect(workspaceShell.getSnapshot().context.objective).toBe("Retry the image")
      expect(workspaceShell.getSnapshot().context.attachments).toEqual([{ path: "pixel.png", expectedRevision: "revision-image" }])
      expect(workspaceShell.getSnapshot().context.error).toBe("The run could not be started. The daemon is still safe to use.")
    } finally {
      bridge.startRun = originalStartRun
      bridge.subscribeRunEvents = originalSubscribeRunEvents
      workspaceShell.stop()
    }
  })

  it("fences a delayed start completion after the selected conversation changes", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      startRun(commandJson: string): Promise<string>
      subscribeRunEvents(sessionId: string, runId: string, callback: (eventJson: string) => void): Promise<string>
    }
    const originalStartRun = bridge.startRun
    const originalSubscribeRunEvents = bridge.subscribeRunEvents
    let resolveStart!: (value: string) => void
    let subscriptions = 0
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "session-old" })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime" } })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { authProfileId: "auth", modelId: "model" } })
      workspaceShell.send({ type: "SET_OBJECTIVE", objective: "Start before changing selection" })
      bridge.startRun = () => new Promise<string>((resolve) => { resolveStart = resolve })
      bridge.subscribeRunEvents = async () => {
        subscriptions += 1
        return JSON.stringify({ events: [] })
      }

      const start = startSelectedRun()
      workspaceShell.send({ type: "SELECTED", sessionId: "session-new" })
      resolveStart(JSON.stringify({ id: "run-old" }))
      await start

      expect(subscriptions).toBe(0)
      expect(workspaceShell.getSnapshot().context.activeRun).toBeUndefined()
      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("session-new")
    } finally {
      bridge.startRun = originalStartRun
      bridge.subscribeRunEvents = originalSubscribeRunEvents
      workspaceShell.stop()
    }
  })

  it("does not subscribe or mutate an obsolete run when selection changes after RUN_STARTED", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      startRun(commandJson: string): Promise<string>
      attachSession(sessionId: string): Promise<string>
      subscribeRunEvents(sessionId: string, runId: string, callback: (eventJson: string) => void): Promise<string>
      releaseRunEventSubscription(): void
    }
    const originalStartRun = bridge.startRun
    const originalAttachSession = bridge.attachSession
    const originalSubscribeRunEvents = bridge.subscribeRunEvents
    const originalRelease = bridge.releaseRunEventSubscription
    const originalInvalidate = desktopQueryClient.invalidateQueries
    let resolveTranscriptInvalidation!: () => void
    let subscriptions = 0
    let releases = 0
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "session-old" })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime" } })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { authProfileId: "auth", modelId: "model" } })
      workspaceShell.send({ type: "SET_OBJECTIVE", objective: "Start then select another conversation" })
      bridge.startRun = async () => JSON.stringify({ id: "run-old" })
      bridge.attachSession = async (sessionId) => JSON.stringify({ id: sessionId, nextRunSelection: { kind: "none" } })
      bridge.subscribeRunEvents = async () => {
        subscriptions += 1
        return JSON.stringify({ events: [] })
      }
      bridge.releaseRunEventSubscription = () => { releases += 1 }
      desktopQueryClient.invalidateQueries = ((filters) => {
        if (filters?.queryKey?.[0] === "daemon" && filters.queryKey[1] === "transcript") {
          return new Promise<void>((resolve) => { resolveTranscriptInvalidation = resolve })
        }
        return originalInvalidate.call(desktopQueryClient, filters)
      }) as typeof desktopQueryClient.invalidateQueries

      const start = startSelectedRun()
      await Promise.resolve()
      await Promise.resolve()
      expect(workspaceShell.getSnapshot().context.activeRun).toBe("run-old")

      await selectConversation("session-new")
      resolveTranscriptInvalidation()
      await start

      expect(subscriptions).toBe(0)
      expect(releases).toBe(1)
      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("session-new")
      expect(workspaceShell.getSnapshot().context.activeRun).toBeUndefined()
      expect(workspaceShell.getSnapshot().context.runDeltas).toEqual([])
    } finally {
      bridge.startRun = originalStartRun
      bridge.attachSession = originalAttachSession
      bridge.subscribeRunEvents = originalSubscribeRunEvents
      bridge.releaseRunEventSubscription = originalRelease
      desktopQueryClient.invalidateQueries = originalInvalidate
      workspaceShell.stop()
    }
  })

  it("keeps the current subscription when an old subscribe completion arrives late", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      startRun(commandJson: string): Promise<string>
      attachSession(sessionId: string): Promise<string>
      subscribeRunEvents(sessionId: string, runId: string, callback: (eventJson: string) => void): Promise<string>
      releaseRunEventSubscription(): void
    }
    const originalStartRun = bridge.startRun
    const originalAttachSession = bridge.attachSession
    const originalSubscribeRunEvents = bridge.subscribeRunEvents
    const originalRelease = bridge.releaseRunEventSubscription
    let markOldSubscribeStarted!: () => void
    const oldSubscribeStarted = new Promise<void>((resolve) => { markOldSubscribeStarted = resolve })
    let resolveOldSubscribe!: (value: string) => void
    let oldCallback!: (eventJson: string) => void
    let releases = 0
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "session-old" })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime" } })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { authProfileId: "auth", modelId: "model" } })
      workspaceShell.send({ type: "SET_OBJECTIVE", objective: "Old run" })
      bridge.startRun = async (commandJson) => JSON.stringify({ id: JSON.parse(commandJson).objective === "Old run" ? "run-old" : "run-current" })
      bridge.attachSession = async (sessionId) => JSON.stringify({ id: sessionId, nextRunSelection: { kind: "none" } })
      bridge.subscribeRunEvents = (_sessionId, runId, callback) => {
        if (runId === "run-old") {
          oldCallback = callback
          markOldSubscribeStarted()
          return new Promise<string>((resolve) => { resolveOldSubscribe = resolve })
        }
        return Promise.resolve(JSON.stringify({ events: [] }))
      }
      bridge.releaseRunEventSubscription = () => { releases += 1 }

      const oldStart = startSelectedRun()
      await oldSubscribeStarted
      await selectConversation("session-current")
      workspaceShell.send({ type: "SET_OBJECTIVE", objective: "Current run" })
      await startSelectedRun()
      expect(workspaceShell.getSnapshot().context.activeRun).toBe("run-current")

      resolveOldSubscribe(JSON.stringify({ events: [assistantDelta("1", "old replay")], latestEventSeq: "1" }))
      await oldStart
      oldCallback(JSON.stringify({
        runId: "run-old",
        payload: { kind: "delta", delta: assistantDelta("2", "old callback") },
      }))

      expect(releases).toBe(1)
      expect(workspaceShell.getSnapshot().context.activeRun).toBe("run-current")
      expect(workspaceShell.getSnapshot().context.transcriptRunId).toBe("run-current")
      expect(workspaceShell.getSnapshot().context.runDeltas).toEqual([])
      expect(workspaceShell.getSnapshot().context.messages).toEqual([])
    } finally {
      bridge.startRun = originalStartRun
      bridge.attachSession = originalAttachSession
      bridge.subscribeRunEvents = originalSubscribeRunEvents
      bridge.releaseRunEventSubscription = originalRelease
      workspaceShell.stop()
    }
  })

  it("triggers a daemon WorkItem only with the complete selected route and attaches its returned run once", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      triggerWorkItem(sessionId: string, paramsJson: string): Promise<string>
      subscribeRunEvents(sessionId: string, runId: string, callback: (eventJson: string) => void): Promise<string>
    }
    const originalTrigger = bridge.triggerWorkItem
    const originalSubscribe = bridge.subscribeRunEvents
    const requests: unknown[] = []
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "session-work-item" })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime-work" } })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { authProfileId: "auth-work", modelId: "model-work" } })
      bridge.triggerWorkItem = async (sessionId, paramsJson) => {
        requests.push({ sessionId, params: JSON.parse(paramsJson) })
        return JSON.stringify({ item: { key: "github:issue-42" }, run: { id: "run-work-item" } })
      }
      bridge.subscribeRunEvents = async () => JSON.stringify({ events: [] })

      await triggerWorkItem("github:issue-42" as never)

      expect(requests).toEqual([{
        sessionId: "session-work-item",
        params: { key: "github:issue-42", selection: { runtimeProfileId: "runtime-work", authProfileId: "auth-work", modelId: "model-work" } },
      }])
      expect(workspaceShell.getSnapshot().context.activeRun).toBe("run-work-item")
    } finally {
      bridge.triggerWorkItem = originalTrigger
      bridge.subscribeRunEvents = originalSubscribe
      workspaceShell.stop()
    }
  })

  it("sends one WorkItem trigger while the desktop interaction is in flight", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      triggerWorkItem(sessionId: string, paramsJson: string): Promise<string>
      subscribeRunEvents(sessionId: string, runId: string, callback: (eventJson: string) => void): Promise<string>
    }
    const originalTrigger = bridge.triggerWorkItem
    const originalSubscribe = bridge.subscribeRunEvents
    let resolveTrigger!: (value: string) => void
    let calls = 0
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "session-work-item" })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime-work" } })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { authProfileId: "auth-work", modelId: "model-work" } })
      bridge.triggerWorkItem = async () => {
        calls += 1
        return new Promise<string>((resolve) => { resolveTrigger = resolve })
      }
      bridge.subscribeRunEvents = async () => JSON.stringify({ events: [] })

      const first = triggerWorkItem("github:issue-42" as never)
      const second = triggerWorkItem("github:issue-42" as never)
      expect(calls).toBe(1)
      resolveTrigger(JSON.stringify({ item: { key: "github:issue-42" }, run: { id: "run-work-item" } }))
      await Promise.all([first, second])

      expect(calls).toBe(1)
      expect(workspaceShell.getSnapshot().context.activeRun).toBe("run-work-item")
    } finally {
      bridge.triggerWorkItem = originalTrigger
      bridge.subscribeRunEvents = originalSubscribe
      workspaceShell.stop()
    }
  })

  it("does not attach a WorkItem run after its selected session changes", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      triggerWorkItem(sessionId: string, paramsJson: string): Promise<string>
      subscribeRunEvents(sessionId: string, runId: string, callback: (eventJson: string) => void): Promise<string>
    }
    const originalTrigger = bridge.triggerWorkItem
    const originalSubscribe = bridge.subscribeRunEvents
    let resolveTrigger!: (value: string) => void
    let subscriptions = 0
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "session-work-item" })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime-work" } })
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: { authProfileId: "auth-work", modelId: "model-work" } })
      bridge.triggerWorkItem = async () => new Promise<string>((resolve) => { resolveTrigger = resolve })
      bridge.subscribeRunEvents = async () => {
        subscriptions += 1
        return JSON.stringify({ events: [] })
      }

      const trigger = triggerWorkItem("github:issue-42" as never)
      workspaceShell.send({ type: "SELECTED", sessionId: "session-other" })
      resolveTrigger(JSON.stringify({ item: { key: "github:issue-42" }, run: { id: "run-work-item" } }))
      await trigger

      expect(workspaceShell.getSnapshot().context.activeRun).toBeUndefined()
      expect(subscriptions).toBe(0)
    } finally {
      bridge.triggerWorkItem = originalTrigger
      bridge.subscribeRunEvents = originalSubscribe
      workspaceShell.stop()
    }
  })

  it("admits Voice runs only after native microphone permission", async () => {
    const requests: string[] = []
    const starts: string[] = []
    const runtime = createDesktopRuntime({
      requestVoicePermission() {
        requests.push("request")
      },
      startRun(commandJson: string) {
        starts.push(commandJson)
        return Promise.resolve(JSON.stringify({ id: "unexpected-run" }))
      },
    } as unknown as NativeDaemonBridge)
    const realtimeSnapshot = {
      runtimeProfiles: [{ id: "runtime-voice", executionKind: "realtimeVoice" }],
    } as unknown as AgentRuntimeSnapshot
    const shellFor = (permission: "notDetermined" | "denied" | "restricted") => {
      const shell = createActor(workspaceShellMachine).start()
      shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      shell.send({ type: "SELECTED", sessionId: `session-${permission}` })
      shell.send({ type: "AGENT_RUNTIME_READY", snapshot: realtimeSnapshot })
      shell.send({ type: "RUNTIME_DRAFT", draft: { runtimeProfileId: "runtime-voice" } })
      shell.send({ type: "SET_OBJECTIVE", objective: "Start voice" })
      shell.send({ type: "VOICE_PERMISSION", permission })
      return shell
    }

    const notDetermined = shellFor("notDetermined")
    await startSelectedRun(runtime, notDetermined)
    expect(requests).toEqual(["request"])
    expect(starts).toEqual([])
    expect(notDetermined.getSnapshot().context.error).toBeUndefined()

    for (const permission of ["denied", "restricted"] as const) {
      const shell = shellFor(permission)
      await startSelectedRun(runtime, shell)
      expect(requests).toEqual(["request"])
      expect(starts).toEqual([])
      expect(shell.getSnapshot().context.error).toBe(
        "Microphone access is required. Grant access in System Settings before starting a voice run.",
      )
      shell.stop()
    }
    notDetermined.stop()
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
