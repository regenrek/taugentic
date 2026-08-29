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
import { archiveConversation, closeTemporaryConversation, createProjectConversation, createSpace, createStandaloneConversation, createTemporaryConversation, desktopRuntime, openProject, selectConversation, setConversationPinned, setProjectSpace, startSelectedRun, triggerWorkItem, workspaceShell, workspaceShellMachine } from "../src/features/runtime/workspace-shell-machine.js"
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

describe("M2 desktop shell ownership", () => {
  it("renders no-project and first-project presentation states without implicit persistence", () => {
    const settingNotifications: string[] = []
    const unsubscribe = desktopSettings.subscribe(() => settingNotifications.push("changed"))
    const noProject = createTestRoot()
    const firstProject = createTestRoot()
    const workspaceId = "workspace-presentation-first-native-render"
    const commands = createCommandDispatcher(desktopSettings, () => ({ canStart: false, canCancel: false }), {
      openSettings() {}, focusPanel() {}, toggleTheme() {}, startRun() {}, cancelRun() {},
    })

    try {
      expect(desktopSettings.presentation(workspaceId)).toBeUndefined()
      expect(workspacePresentation(workspaceId)).toEqual({
        theme: desktopSettings.appearance().theme,
        layout: defaultWorkspaceLayout,
      })

      noProject.render(<QueryClientProvider client={desktopQueryClient}><App /></QueryClientProvider>)
      expect(noProject.renderer.findByTestId("workspace-awaiting-project")).toBeDefined()

      firstProject.render(<Workbench
        workspaceId={workspaceId}
        presentation={workspacePresentation(workspaceId)}
        panels={[{ id: "conversation", label: "Conversation", content: <div /> }]}
        commands={commands}
      />)
      expect(firstProject.renderer.findByTestId("workspace-dock")).toBeDefined()
      expect(desktopSettings.presentation(workspaceId)).toBeUndefined()
      expect(settingNotifications).toEqual([])
    } finally {
      noProject.unmount()
      firstProject.unmount()
      unsubscribe()
    }
  })

  it("selects the exact Scheduled Work linked run through Run Activity before focusing Activity", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      getRun(sessionId: string, queryJson: string): Promise<string>
      runTimeline(sessionId: string, queryJson: string): Promise<string>
      listApprovals(queryJson: string): Promise<string>
      replayRunEvents(sessionId: string, queryJson: string): Promise<string>
    }
    const originalGetRun = bridge.getRun
    const originalRunTimeline = bridge.runTimeline
    const originalListApprovals = bridge.listApprovals
    const originalReplayRunEvents = bridge.replayRunEvents
    const selectedRunQueries: unknown[] = []
    const root = createTestRoot()
    const sessionId = "session-scheduled"
    const linkedRunId = "run-scheduled"
    workspaceShell.start()
    try {
      desktopQueryClient.clear()
      desktopQueryClient.setQueryData(navigationQueryKey, {
        spaces: [], agents: [], projects: [],
        conversations: [{ sessionId, workspaceId: "workspace-scheduled", title: "Scheduled conversation", status: "idle", placement: { kind: "standalone" }, archived: false, pinned: false }],
      })
      desktopQueryClient.setQueryData(scheduledWorkQueryKey(sessionId as never), {
        occurrences: [{ id: "occurrence-scheduled", scheduledWorkId: "scheduled-one", dueAtMs: "1780000000000", state: { kind: "claimed", run_id: linkedRunId } }],
      })
      desktopQueryClient.setQueryData([...runActivityQueryRoot, sessionId, "runs", { limit: 100 }], {
        runs: [
          { id: "run-first", relationship: { kind: "root" }, harness: "native", status: "completed", objectivePreview: "First run" },
          { id: linkedRunId, relationship: { kind: "root" }, harness: "native", status: "completed", objectivePreview: "Scheduled run" },
        ],
      })
      bridge.getRun = async (_sessionId, queryJson) => {
        selectedRunQueries.push(JSON.parse(queryJson))
        return "null"
      }
      bridge.runTimeline = async () => "null"
      bridge.listApprovals = async () => JSON.stringify({ items: [] })
      bridge.replayRunEvents = async () => JSON.stringify({ events: [] })
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId })
      root.render(<QueryClientProvider client={desktopQueryClient}><App /></QueryClientProvider>)
      await Promise.resolve()
      await Promise.resolve()
      selectedRunQueries.length = 0

      const open = root.renderer.findByTestId("open-scheduled-work-run-occurrence-scheduled")!
      const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(open.id) ?? []
      root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)
      await Promise.resolve()
      await Promise.resolve()

      expect(selectedRunQueries).toContainEqual({ runId: linkedRunId })
      expect(workspaceShell.getSnapshot().context.focusPanelId).toBe("activity")
    } finally {
      bridge.getRun = originalGetRun
      bridge.runTimeline = originalRunTimeline
      bridge.listApprovals = originalListApprovals
      bridge.replayRunEvents = originalReplayRunEvents
      root.unmount()
      desktopQueryClient.clear()
      workspaceShell.stop()
    }
  })

  it("derives the workbench from the selected daemon conversation without changing project browsing", () => {
    const navigation = {
      spaces: [], agents: [],
      projects: [
        { id: "project-browsing", title: "Browsing", workspaceIds: ["workspace-browsing"] },
        { id: "project-conversation", title: "Conversation", workspaceIds: ["workspace-conversation"] },
      ],
      conversations: [
        { sessionId: "standalone", workspaceId: "workspace-standalone", title: "Standalone", status: "idle", placement: { kind: "standalone" }, archived: false, pinned: false },
        { sessionId: "project-thread", workspaceId: "workspace-thread", title: "Project thread", status: "idle", placement: { kind: "project", projectId: "project-conversation" }, archived: false, pinned: false },
      ],
    } as never

    const standalone = workbenchSelection(navigation, "project-browsing", "standalone")
    expect(standalone.browsingProject?.id).toBe("project-browsing")
    expect(standalone.project).toBeUndefined()
    expect(standalone.workspaceId).toBe("workspace-standalone")

    const projectConversation = workbenchSelection(navigation, "project-browsing", "project-thread")
    expect(projectConversation.browsingProject?.id).toBe("project-browsing")
    expect(projectConversation.project?.id).toBe("project-conversation")
    expect(projectConversation.workspaceId).toBe("workspace-thread")

    const browsingOnly = workbenchSelection(navigation, "project-browsing")
    expect(browsingOnly.project?.id).toBe("project-browsing")
    expect(browsingOnly.workspaceId).toBe("workspace-browsing")
  })

  it("makes project trust keyboard-operable with cancellation focused by default", () => {
    const decisions: boolean[] = []
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(<ProjectTrustConfirmation onDecision={(acknowledged) => decisions.push(acknowledged)} />)

      renderer.simulateKeystrokes("enter tab enter escape")

      expect(decisions).toEqual([false, true, false])
    } finally {
      unmount()
    }
  })

  it("keeps the runtime route compact until the user opens its chooser", () => {
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(<RuntimeRoutePicker snapshot={undefined} draft={undefined} pendingAuthMethodIds={[]} onDraft={() => {}} onLogin={() => {}} onLogout={() => {}} onPreferences={() => {}} />)
      expect(renderer.findByTestId("runtime-route-summary")).toBeDefined()
      expect(renderer.findByTestId("runtime-route-options")).toBeUndefined()
      const toggle = renderer.findByTestId("runtime-route-toggle")!
      const bounds = renderer.getElementBounds(toggle.id)
      expect(bounds).not.toBeNull()
      const [x = 0, y = 0, width = 0, height = 0] = bounds ?? []
      renderer.nativeSimulateClick(x + width / 2, y + height / 2)
      expect(renderer.findByTestId("runtime-route-options")).toBeDefined()
    } finally {
      unmount()
    }
  })

  it("presents only the selected provider and auth-method account group with complete preferences", () => {
    const preferences: Array<{ id: string, value: unknown }> = []
    const { render, renderer, unmount } = createTestRoot()
    const snapshot = {
      providers: [{ id: "codex", displayName: "Codex", models: [] }],
      authMethods: [
        { id: "chatgpt", providerId: "codex", displayName: "ChatGPT" },
        { id: "api-key", providerId: "codex", displayName: "API key" },
      ],
      runtimeProfiles: [{ id: "codex-safe", displayName: "Codex Safe", providerId: "codex", authMethodId: "chatgpt", policyMode: "requireApproval" }],
      authProfiles: [
        { profile: { id: "account-a", authMethodId: "chatgpt", providerId: "codex", displayName: "Account A", planTier: "Pro" }, preferences: { label: "Personal", order: 0, isDefault: true }, usage: { kind: "observed", observedAtMs: "1", windows: [{ label: "weekly", remaining: "20", limit: "40" }] }, connectionState: "connected", exhaustion: null, canLogin: false, canLogout: true },
        { profile: { id: "account-b", authMethodId: "chatgpt", providerId: "codex", displayName: "Account B", planTier: "Team" }, preferences: { label: "Work", order: 1, isDefault: false }, usage: { kind: "unavailable" }, connectionState: "connected", exhaustion: "rateLimited", canLogin: false, canLogout: true },
        { profile: { id: "wrong-method", authMethodId: "api-key", providerId: "codex", displayName: "Wrong method", planTier: "Enterprise" }, preferences: { label: "Hidden", order: 0, isDefault: true }, usage: { kind: "unavailable" }, connectionState: "connected", exhaustion: null, canLogin: false, canLogout: true },
      ],
    } as unknown as AgentRuntimeSnapshot
    const click = (testId: string) => {
      const element = renderer.findByTestId(testId)!
      const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id) ?? []
      renderer.nativeSimulateClick(x + width / 2, y + height / 2)
    }

    try {
      render(<RuntimeRoutePicker snapshot={snapshot} draft={{ runtimeProfileId: "codex-safe", authProfileId: "account-a" }} pendingAuthMethodIds={[]} onDraft={() => {}} onLogin={() => {}} onLogout={() => {}} onPreferences={(id, value) => preferences.push({ id, value })} />)
      click("runtime-route-toggle")

      const text = renderer.getAllText().join(" ").replace(/\s+/g, " ")
      expect(text).toContain("Personal · default · connected · Pro · weekly: 20/40 · order 1")
      expect(text).toContain("Work · connected · Team · usage unavailable · rateLimited · order 2")
      expect(text).not.toContain("Hidden")
      expect(renderer.findByTestId("login-auth-method-chatgpt")).toBeDefined()
      expect(renderer.findByTestId("login-auth-method-api-key")).toBeUndefined()

      click("default-auth-profile-account-b")
      click("move-auth-profile-up-account-b")
      click("move-auth-profile-down-account-a")
      expect(preferences).toEqual([
        { id: "account-b", value: { label: "Work", order: 1, isDefault: true } },
        { id: "account-b", value: { label: "Work", order: 0, isDefault: false } },
        { id: "account-a", value: { label: "Personal", order: 1, isDefault: true } },
      ])
    } finally {
      unmount()
    }
  })

  it("keeps one selector state and concrete workspace presentation", () => {
    const initial: SidebarState = { view: "spaces", filter: "", expandedSpaceIds: [] }
    const selected = sidebarReduce(initial, { type: "selectView", view: "agents" })
    expect(selected.view).toBe("agents")

    const workspaceId = "workspace-m2-test"
    expect(desktopSettings.presentation(workspaceId)).toBeUndefined()
    desktopSettings.saveLayout(workspaceId, { kind: "tabs", id: "root", panels: ["conversation"], active: "conversation" })
    expect(desktopSettings.presentation(workspaceId)?.layout.kind).toBe("tabs")
    expect(commandRegistry.map((command) => command.id)).toEqual(["open-settings", "focus-conversation", "focus-activity", "focus-thread-workspace", "focus-git", "focus-pull-requests", "focus-terminal", "toggle-theme", "start-run", "cancel-run"])
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
        { sessionId: "conversation-a", workspaceId: "workspace-a", title: "A", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: false },
        { sessionId: "conversation-b", workspaceId: "workspace-b", title: "B", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-b" }, archived: false, pinned: false },
        { sessionId: "conversation-c", workspaceId: "workspace-c", title: "C", status: "idle" as const, placement: { kind: "standalone" as const }, archived: false, pinned: false },
      ],
    }

    expect(projectConversations(snapshot, "project-b").map((conversation) => conversation.sessionId)).toEqual(["conversation-b"])
    expect(projectConversations(snapshot)).toEqual([])
  })

  it("keeps active and archived project conversations in the one daemon navigation projection", () => {
    const snapshot = {
      spaces: [], projects: [], agents: [],
      conversations: [
        { sessionId: "active", workspaceId: "workspace-a", title: "Active", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: true },
        { sessionId: "archived", workspaceId: "workspace-a", title: "Archived", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: true, pinned: false },
      ],
    }

    expect(projectConversations(snapshot, "project-a").map((conversation) => conversation.sessionId)).toEqual(["active"])
    expect(archivedProjectConversations(snapshot, "project-a").map((conversation) => conversation.sessionId)).toEqual(["archived"])
  })

  it("projects standalone and temporary conversations from the one daemon navigation snapshot", () => {
    const snapshot = {
      spaces: [], projects: [], agents: [],
      conversations: [
        { sessionId: "standalone", workspaceId: "workspace-a", title: "Standalone", status: "idle" as const, placement: { kind: "standalone" as const }, archived: false, pinned: false },
        { sessionId: "temporary", workspaceId: "workspace-a", title: "Temporary", status: "completed" as const, placement: { kind: "temporary" as const }, archived: false, pinned: false },
        { sessionId: "project", workspaceId: "workspace-a", title: "Project", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: false },
      ],
    }

    expect(standaloneConversations(snapshot).map((conversation) => conversation.sessionId)).toEqual(["standalone"])
    expect(temporaryConversations(snapshot).map((conversation) => conversation.sessionId)).toEqual(["temporary"])
  })

  it("uses an isolated daemon search query and renders every matched conversation placement", async () => {
    const searches: Array<string | undefined> = []
    const resultSnapshot = {
      spaces: [], projects: [], agents: [],
      conversations: [
        { sessionId: "project-result", workspaceId: "workspace-a", title: "Needle project", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: false },
        { sessionId: "standalone-result", workspaceId: "workspace-a", title: "Needle standalone", status: "idle" as const, placement: { kind: "standalone" as const }, archived: false, pinned: false },
        { sessionId: "temporary-result", workspaceId: "workspace-a", title: "Needle temporary", status: "idle" as const, placement: { kind: "temporary" as const }, archived: false, pinned: false },
        { sessionId: "archived-result", workspaceId: "workspace-a", title: "Needle archived", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: true, pinned: false },
      ],
    }
    const runtime = createDesktopRuntime({
      navigationSnapshot(search?: string) {
        searches.push(search)
        return Promise.resolve(JSON.stringify(resultSnapshot))
      },
    } as unknown as NativeDaemonBridge)
    const query = navigationQuery(runtime, " Needle ")
    const selected: string[] = []
    const restored: string[] = []
    const { render, renderer, unmount } = createTestRoot()

    try {
      expect([...query.queryKey]).toEqual([...navigationQueryKey, " Needle "])
      expect([...navigationQuery(runtime).queryKey]).toEqual([...navigationQueryKey])
      await query.queryFn!({ queryKey: query.queryKey } as never)
      expect(searches).toEqual([" Needle "])

      render(<Sidebar
        snapshot={resultSnapshot}
        state={{ view: "spaces", filter: " Needle ", expandedSpaceIds: [] }}
        conversationTitle=""
        canCreateConversation={false}
        canOrganizeConversations
        searchMode
        dispatch={(action) => { if (action.type === "selectConversation") selected.push(action.sessionId) }}
        onConversationTitleChange={() => {}}
        onCreateConversation={() => {}}
        onRestoreConversation={(sessionId) => restored.push(sessionId)}
      />)
      expect(renderer.findByTestId("sidebar-search-results")).toBeDefined()
      for (const id of ["project-result", "standalone-result", "temporary-result", "archived-result"]) {
        expect(renderer.findByTestId(`conversation-entry-${id}`)).toBeDefined()
      }
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("conversation-entry-temporary-result")!.id, "space")
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("restore-conversation-archived-result")!.id, "space")
      expect(selected).toEqual(["temporary-result"])
      expect(restored).toEqual(["archived-result"])
    } finally {
      unmount()
    }
  })

  it("exposes temporary close only for idle or terminal sessions with an accessible disabled paused control", () => {
    const closed: string[] = []
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(<Sidebar
        snapshot={{
          spaces: [], projects: [], agents: [],
          conversations: [
            { sessionId: "temporary-idle", workspaceId: "workspace-a", title: "Idle temporary", status: "idle", placement: { kind: "temporary" }, archived: false, pinned: false },
            { sessionId: "temporary-paused", workspaceId: "workspace-a", title: "Paused temporary", status: "paused", placement: { kind: "temporary" }, archived: false, pinned: false },
          ],
        }}
        state={{ view: "spaces", filter: "", expandedSpaceIds: [] }}
        conversationTitle=""
        canCreateConversation={false}
        canOrganizeConversations
        onConversationTitleChange={() => {}}
        onCreateConversation={() => {}}
        onCloseTemporaryConversation={(sessionId) => closed.push(sessionId)}
        dispatch={() => {}}
      />)
      const click = (testId: string) => {
        const element = renderer.findByTestId(testId)!
        const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id) ?? []
        renderer.nativeSimulateClick(x + width / 2, y + height / 2)
      }
      click("close-temporary-conversation-temporary-idle")
      click("close-temporary-conversation-temporary-paused")

      expect(closed).toEqual(["temporary-idle"])
      expect(renderer.getAutomationTree()).toContain("Close temporary conversation Paused temporary")
    } finally {
      unmount()
    }
  })

  it("renders pinned conversations first and makes pin, unpin, archive, and restore controls keyboard-accessible", () => {
    const actions: string[] = []
    const { render, renderer, unmount } = createTestRoot()
    const click = (testId: string) => {
      const element = renderer.findByTestId(testId)!
      const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id) ?? []
      renderer.nativeSimulateClick(x + width / 2, y + height / 2)
    }
    const snapshot = {
      spaces: [], projects: [], agents: [],
      conversations: [
        { sessionId: "active", workspaceId: "workspace-a", title: "Active", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: false },
        { sessionId: "pinned", workspaceId: "workspace-a", title: "Pinned", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: true },
        { sessionId: "archived", workspaceId: "workspace-a", title: "Archived", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: true, pinned: false },
      ],
    }
    try {
      expect(projectConversations(snapshot, "project-a").map((conversation) => conversation.sessionId)).toEqual(["pinned", "active"])
      render(<Sidebar snapshot={snapshot} state={{ view: "projects", filter: "", expandedSpaceIds: [] }} selectedProjectId="project-a" conversationTitle="" canCreateConversation={false} canOrganizeConversations onConversationTitleChange={() => {}} onCreateConversation={() => {}} onSetPinnedConversation={(id, pinned) => actions.push(`${pinned ? "pin" : "unpin"}:${id}`)} onArchiveConversation={(id) => actions.push(`archive:${id}`)} onRestoreConversation={(id) => actions.push(`restore:${id}`)} dispatch={() => {}} />)
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("pin-conversation-active")!.id, "space")
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("pin-conversation-pinned")!.id, "space")
      click("archive-conversation-active")
      expect(actions).toEqual(["pin:active", "unpin:pinned", "archive:active"])
      expect(renderer.getAutomationTree()).toContain('"name":"Archive conversation Active"')
      expect(renderer.getAutomationTree()).toContain('"name":"Unpin conversation Pinned"')

      render(<Sidebar snapshot={snapshot} state={{ view: "archived", filter: "", expandedSpaceIds: [] }} selectedProjectId="project-a" conversationTitle="" canCreateConversation={false} canOrganizeConversations onConversationTitleChange={() => {}} onCreateConversation={() => {}} onRestoreConversation={(id) => actions.push(`restore:${id}`)} dispatch={() => {}} />)
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("restore-conversation-archived")!.id, "space")
      expect(actions).toEqual(["pin:active", "unpin:pinned", "archive:active", "restore:archived"])
      expect(renderer.getAutomationTree()).toContain('"name":"Restore conversation Archived"')
    } finally { unmount() }
  })

  it("serializes navigation intent through the typed desktop runtime adapter", async () => {
    const requests: unknown[] = []
    const runtime = createDesktopRuntime({
      navigationIntent(intentJson: string) {
        requests.push(JSON.parse(intentJson))
        return Promise.resolve(JSON.stringify({ spaces: [], projects: [], agents: [], conversations: [] }))
      },
    } as unknown as NativeDaemonBridge)

    await runtime.navigationIntent({ kind: "setArchived", sessionId: "conversation-a", archived: true })
    await runtime.navigationIntent({ kind: "setArchived", sessionId: "conversation-a", archived: false })
    await runtime.navigationIntent({ kind: "setPinned", sessionId: "conversation-a", pinned: false })

    expect(requests).toEqual([
      { kind: "setArchived", sessionId: "conversation-a", archived: true },
      { kind: "setArchived", sessionId: "conversation-a", archived: false },
      { kind: "setPinned", sessionId: "conversation-a", pinned: false },
    ])
  })

  it("treats sidebar conversation selection as an attach intent until the real App dispatch succeeds", async () => {
    const bridge = desktopRuntime.bridge as unknown as { attachSession(sessionId: string): Promise<string> }
    const originalAttach = bridge.attachSession
    let resolveAttach!: (value: string) => void
    const snapshot = {
      spaces: [], agents: [],
      projects: [{ id: "project-a", title: "Project A", workspaceIds: ["workspace-a"] }],
      conversations: [
        { sessionId: "old", title: "Old", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: false },
        { sessionId: "target", title: "Target", status: "idle" as const, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: false },
      ],
    }
    const root = createTestRoot()
    workspaceShell.start()
    try {
      desktopQueryClient.setQueryData(navigationQueryKey, snapshot)
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "PROJECT_SELECTED", projectId: "project-a" })
      workspaceShell.send({ type: "SELECTED", sessionId: "old" })
      bridge.attachSession = () => new Promise<string>((resolve) => { resolveAttach = resolve })
      root.render(<QueryClientProvider client={desktopQueryClient}><App /></QueryClientProvider>)
      const entry = root.renderer.findByTestId("conversation-entry-target")!
      const sidebar = root.renderer.findByTestId("workspace-sidebar")!
      const [sidebarX = 0, sidebarY = 0, sidebarWidth = 0, sidebarHeight = 0] = root.renderer.getElementBounds(sidebar.id) ?? []
      const [, entryY = 0, , entryHeight = 0] = root.renderer.getElementBounds(entry.id) ?? []
      root.renderer.nativeSimulateScrollWheel(
        sidebarX + sidebarWidth / 2,
        sidebarY + sidebarHeight / 2,
        0,
        -Math.max(1, entryY + entryHeight - (sidebarY + sidebarHeight)),
      )
      const [x = 0, y = 0, width = 0, height = 0] = root.renderer.getElementBounds(entry.id) ?? []
      root.renderer.nativeSimulateClick(x + width / 2, y + height / 2)

      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("old")
      resolveAttach(JSON.stringify({ id: "target", nextRunSelection: { kind: "none" } }))
      await Promise.resolve()
      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("target")
    } finally {
      bridge.attachSession = originalAttach
      root.unmount()
      workspaceShell.stop()
    }
  })

  it("keeps the newest attachSession selection when an older attach completes last", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      attachSession(sessionId: string): Promise<string>
      releaseRunEventSubscription(): void
    }
    const originalAttach = bridge.attachSession
    const originalRelease = bridge.releaseRunEventSubscription
    let resolveFirst!: (value: string) => void
    let resolveSecond!: (value: string) => void
    let releases = 0
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      bridge.releaseRunEventSubscription = () => { releases += 1 }
      bridge.attachSession = (sessionId) => new Promise<string>((resolve) => {
        if (sessionId === "first") resolveFirst = resolve
        else resolveSecond = resolve
      })

      const first = selectConversation("first")
      const second = selectConversation("second")
      resolveSecond(JSON.stringify({ id: "second", nextRunSelection: { kind: "none" } }))
      await second
      resolveFirst(JSON.stringify({ id: "first", nextRunSelection: { kind: "none" } }))
      await first

      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("second")
      expect(releases).toBe(1)
    } finally {
      bridge.attachSession = originalAttach
      bridge.releaseRunEventSubscription = originalRelease
      workspaceShell.stop()
    }
  })

  it("admits one transient organization mutation and clears only the archived current selection", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      navigationIntent(intentJson: string): Promise<string>
      releaseRunEventSubscription(): void
    }
    const originalIntent = bridge.navigationIntent
    const originalRelease = bridge.releaseRunEventSubscription
    let resolveIntent!: (value: string) => void
    const requests: unknown[] = []
    let releases = 0
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "conversation-current" })
      bridge.releaseRunEventSubscription = () => { releases += 1 }
      bridge.navigationIntent = (intentJson) => {
        requests.push(JSON.parse(intentJson))
        return new Promise<string>((resolve) => { resolveIntent = resolve })
      }

      const archive = archiveConversation("conversation-current")
      const pin = setConversationPinned("conversation-current", true)
      expect(requests).toEqual([{ kind: "setArchived", sessionId: "conversation-current", archived: true }])
      resolveIntent(JSON.stringify({ spaces: [], projects: [], agents: [], conversations: [] }))
      await Promise.all([archive, pin])

      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBeUndefined()
      expect(releases).toBe(1)
    } finally {
      bridge.navigationIntent = originalIntent
      bridge.releaseRunEventSubscription = originalRelease
      workspaceShell.stop()
    }
  })

  it("uses the existing organization mutation and navigation epoch owner for spaces and project placement", async () => {
    const bridge = desktopRuntime.bridge as unknown as { navigationIntent(intentJson: string): Promise<string> }
    const originalIntent = bridge.navigationIntent
    const requests: unknown[] = []
    const snapshot = {
      spaces: [{ id: "space-product", title: "Product" }],
      projects: [{ id: "project-desktop", spaceId: "space-product", title: "Desktop", workspaceIds: ["workspace-desktop"] }],
      agents: [],
      conversations: [],
    }
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      bridge.navigationIntent = async (intentJson) => {
        requests.push(JSON.parse(intentJson))
        return JSON.stringify(snapshot)
      }

      expect(await createSpace(" Product ")).toBe(true)
      expect(await setProjectSpace("project-desktop", "space-product")).toBe(true)
      expect(await setProjectSpace("project-desktop")).toBe(true)
      expect(requests).toEqual([
        { kind: "createSpace", title: "Product" },
        { kind: "setProjectSpace", projectId: "project-desktop", spaceId: "space-product" },
        { kind: "setProjectSpace", projectId: "project-desktop" },
      ])
      expect(desktopQueryClient.getQueryData<unknown>(navigationQueryKey)).toEqual(snapshot)
    } finally {
      bridge.navigationIntent = originalIntent
      workspaceShell.stop()
    }
  })

  it("fences a stale organization response when opening a project begins", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      navigationIntent(intentJson: string): Promise<string>
      openProject(path: string, trustAcknowledged: boolean): Promise<string>
    }
    const originalIntent = bridge.navigationIntent
    const originalOpen = bridge.openProject
    let resolveIntent!: (value: string) => void
    let resolveOpen!: (value: string) => void
    const baseline = { spaces: [], projects: [], agents: [], conversations: [{ sessionId: "baseline", title: "Baseline", status: "idle", placement: { kind: "standalone" }, archived: false, pinned: false }] }
    workspaceShell.start()
    try {
      desktopQueryClient.setQueryData(navigationQueryKey, baseline)
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      bridge.navigationIntent = () => new Promise<string>((resolve) => { resolveIntent = resolve })
      bridge.openProject = () => new Promise<string>((resolve) => { resolveOpen = resolve })

      const pin = setConversationPinned("conversation-stale", true)
      const open = openProject("/project", true)
      resolveIntent(JSON.stringify({ spaces: [], projects: [], agents: [], conversations: [{ sessionId: "stale", title: "Stale", status: "idle", placement: { kind: "standalone" }, archived: false, pinned: true }] }))
      await pin
      expect(desktopQueryClient.getQueryData<unknown>(navigationQueryKey)).toEqual(baseline)
      resolveOpen(JSON.stringify({ projectId: "project-opened", snapshot: baseline }))
      await open
    } finally {
      bridge.navigationIntent = originalIntent
      bridge.openProject = originalOpen
      workspaceShell.stop()
    }
  })

  it("publishes only the newest concurrent openProject completion", async () => {
    const bridge = desktopRuntime.bridge as unknown as { openProject(path: string, trustAcknowledged: boolean): Promise<string> }
    const originalOpen = bridge.openProject
    let resolveFirst!: (value: string) => void
    let resolveSecond!: (value: string) => void
    const firstSnapshot = { spaces: [], projects: [{ id: "project-first", title: "First", workspaceIds: ["workspace-first"] }], agents: [], conversations: [] }
    const secondSnapshot = { spaces: [], projects: [{ id: "project-second", title: "Second", workspaceIds: ["workspace-second"] }], agents: [], conversations: [] }
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      bridge.openProject = (path) => new Promise<string>((resolve) => {
        if (path === "/first") resolveFirst = resolve
        else resolveSecond = resolve
      })

      const first = openProject("/first", true)
      const second = openProject("/second", true)
      resolveSecond(JSON.stringify({ projectId: "project-second", snapshot: secondSnapshot }))
      await second
      resolveFirst(JSON.stringify({ projectId: "project-first", snapshot: firstSnapshot }))
      await first

      expect(desktopQueryClient.getQueryData<unknown>(navigationQueryKey)).toEqual(secondSnapshot)
      expect(workspaceShell.getSnapshot().context.selectedProjectId).toBe("project-second")
    } finally {
      bridge.openProject = originalOpen
      workspaceShell.stop()
    }
  })

  it("fences a stale organization response when creating a project conversation begins", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      navigationIntent(intentJson: string): Promise<string>
      openSession(paramsJson: string): Promise<string>
    }
    const originalIntent = bridge.navigationIntent
    const originalOpenSession = bridge.openSession
    let resolveIntent!: (value: string) => void
    let resolveSession!: (value: string) => void
    const baseline = { spaces: [], projects: [], agents: [], conversations: [{ sessionId: "baseline", title: "Baseline", status: "idle", placement: { kind: "standalone" }, archived: false, pinned: false }] }
    workspaceShell.start()
    try {
      desktopQueryClient.setQueryData(navigationQueryKey, baseline)
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      bridge.navigationIntent = () => new Promise<string>((resolve) => { resolveIntent = resolve })
      bridge.openSession = () => new Promise<string>((resolve) => { resolveSession = resolve })

      const pin = setConversationPinned("conversation-stale", true)
      const create = createProjectConversation("project-a", "workspace-a", "Created")
      resolveIntent(JSON.stringify({ spaces: [], projects: [], agents: [], conversations: [{ sessionId: "stale", title: "Stale", status: "idle", placement: { kind: "standalone" }, archived: false, pinned: true }] }))
      await pin
      expect(desktopQueryClient.getQueryData<unknown>(navigationQueryKey)).toEqual(baseline)
      resolveSession(JSON.stringify({ id: "created", nextRunSelection: { kind: "none" } }))
      await create
    } finally {
      bridge.navigationIntent = originalIntent
      bridge.openSession = originalOpenSession
      workspaceShell.stop()
    }
  })

  it("commits only the newest concurrent project conversation creation", async () => {
    const bridge = desktopRuntime.bridge as unknown as { openSession(paramsJson: string): Promise<string> }
    const originalOpenSession = bridge.openSession
    let resolveFirst!: (value: string) => void
    let resolveSecond!: (value: string) => void
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      bridge.openSession = (paramsJson) => new Promise<string>((resolve) => {
        if (JSON.parse(paramsJson).title === "First") resolveFirst = resolve
        else resolveSecond = resolve
      })

      const first = createProjectConversation("project-a", "workspace-a", "First")
      const second = createProjectConversation("project-a", "workspace-a", "Second")
      resolveSecond(JSON.stringify({ id: "second", nextRunSelection: { kind: "none" } }))
      expect(await second).toBe(true)
      resolveFirst(JSON.stringify({ id: "first", nextRunSelection: { kind: "none" } }))
      expect(await first).toBe(false)

      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("second")
    } finally {
      bridge.openSession = originalOpenSession
      workspaceShell.stop()
    }
  })

  it("opens standalone conversations by workspace id and keeps only the current temporary creation selected", async () => {
    const bridge = desktopRuntime.bridge as unknown as { openSession(paramsJson: string): Promise<string> }
    const originalOpenSession = bridge.openSession
    const requests: unknown[] = []
    let resolveFirstTemporary!: (value: string) => void
    let resolveSecondTemporary!: (value: string) => void
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      bridge.openSession = (paramsJson) => {
        const params = JSON.parse(paramsJson)
        requests.push(params)
        if (params.title === "Standalone") return Promise.resolve(JSON.stringify({ id: "standalone", nextRunSelection: { kind: "none" } }))
        return new Promise<string>((resolve) => {
          if (params.title === "First temporary") resolveFirstTemporary = resolve
          else resolveSecondTemporary = resolve
        })
      }

      expect(await createStandaloneConversation("workspace-standalone", " Standalone ")).toBe(true)
      const first = createTemporaryConversation("workspace-temporary", "First temporary")
      const second = createTemporaryConversation("workspace-temporary", "Second temporary")
      resolveSecondTemporary(JSON.stringify({ id: "temporary-second", nextRunSelection: { kind: "none" } }))
      expect(await second).toBe(true)
      resolveFirstTemporary(JSON.stringify({ id: "temporary-first", nextRunSelection: { kind: "none" } }))
      expect(await first).toBe(false)

      expect(requests).toEqual([
        { title: "Standalone", workspace: { kind: "byId", id: "workspace-standalone" } },
        { title: "First temporary", workspace: { kind: "byTemporary", workspaceId: "workspace-temporary" } },
        { title: "Second temporary", workspace: { kind: "byTemporary", workspaceId: "workspace-temporary" } },
      ])
      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("temporary-second")
    } finally {
      bridge.openSession = originalOpenSession
      workspaceShell.stop()
    }
  })

  it("closes the selected temporary conversation through its one navigation intent", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      navigationIntent(intentJson: string): Promise<string>
      releaseRunEventSubscription(): void
    }
    const originalIntent = bridge.navigationIntent
    const originalRelease = bridge.releaseRunEventSubscription
    const requests: unknown[] = []
    let releases = 0
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "temporary-current" })
      bridge.navigationIntent = async (intentJson) => {
        requests.push(JSON.parse(intentJson))
        return JSON.stringify({ spaces: [], projects: [], agents: [], conversations: [] })
      }
      bridge.releaseRunEventSubscription = () => { releases += 1 }

      await closeTemporaryConversation("temporary-current")

      expect(requests).toEqual([{ kind: "closeTemporaryConversation", sessionId: "temporary-current" }])
      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBeUndefined()
      expect(releases).toBe(1)
    } finally {
      bridge.navigationIntent = originalIntent
      bridge.releaseRunEventSubscription = originalRelease
      workspaceShell.stop()
    }
  })

  it("invalidates a pending attach when its target is archived", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      attachSession(sessionId: string): Promise<string>
      navigationIntent(intentJson: string): Promise<string>
    }
    const originalAttach = bridge.attachSession
    const originalIntent = bridge.navigationIntent
    let resolveAttach!: (value: string) => void
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "current" })
      bridge.attachSession = () => new Promise<string>((resolve) => { resolveAttach = resolve })
      bridge.navigationIntent = async () => JSON.stringify({ spaces: [], projects: [], agents: [], conversations: [] })

      const attach = selectConversation("pending")
      await archiveConversation("pending")
      resolveAttach(JSON.stringify({ id: "pending", nextRunSelection: { kind: "none" } }))
      await attach

      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("current")
    } finally {
      bridge.attachSession = originalAttach
      bridge.navigationIntent = originalIntent
      workspaceShell.stop()
    }
  })

  it("keeps long project conversation lists reachable through the sidebar scroll owner", () => {
    const { render, renderer, unmount } = createTestRoot()
    const projectId = "project-scroll-owner"
    const conversations = Array.from({ length: 24 }, (_, index) => ({
      sessionId: `conversation-${index}`,
      workspaceId: "workspace-scroll-owner",
      title: `Conversation ${index}`,
      status: "idle" as const,
      placement: { kind: "project" as const, projectId },
      archived: false,
      pinned: false,
    }))

    try {
      render(<div style={{ display: "flex", width: 320, height: 240 }}>
        <Sidebar
          snapshot={{ spaces: [], projects: [], agents: [], conversations }}
          state={{ view: "spaces", filter: "", expandedSpaceIds: [] }}
          selectedProjectId={projectId}
          conversationTitle=""
          canCreateConversation={false}
          dispatch={() => {}}
          onConversationTitleChange={() => {}}
          onCreateConversation={() => {}}
        />
      </div>)
      const sidebar = renderer.findByTestId("workspace-sidebar")
      expect(sidebar).toBeDefined()
      expect(renderer.getScrollOffset(sidebar!.id)).toEqual([0, 0])

      renderer.nativeSimulateScrollWheel(160, 120, 0, -120)

      expect(renderer.getScrollOffset(sidebar!.id)?.[1]).toBeLessThan(0)
    } finally {
      unmount()
    }
  })

  it("uses one conversation-creation availability fact for button and Enter", () => {
    const created: string[] = []
    const { render, renderer, unmount } = createTestRoot()
    try {
      const props = { snapshot: { spaces: [], projects: [], agents: [], conversations: [] }, state: { view: "spaces" as const, filter: "", expandedSpaceIds: [] }, selectedProjectId: "project-create", conversationTitle: "New thread", dispatch() {}, onConversationTitleChange() {}, onCreateConversation: () => created.push("created") }
      render(<Sidebar {...props} canCreateConversation={false} />)
      const title = renderer.findByTestId("new-conversation-title")!
      const button = renderer.findByTestId("create-conversation")!
      expect(renderer.getAutomationTree()).toContain('"name":"New conversation","disabled":true')
      renderer.nativeSimulateKeystrokes(title.id, "enter")
      renderer.nativeSimulateKeystrokes(button.id, "enter")
      expect(created).toEqual([])
      render(<Sidebar {...props} canCreateConversation />)
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("new-conversation-title")!.id, "enter")
      expect(created).toEqual(["created"])
    } finally { unmount() }
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
