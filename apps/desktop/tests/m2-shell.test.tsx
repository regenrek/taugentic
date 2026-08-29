import { createActor } from "xstate"
import { describe, expect, it } from "bun:test"
import { createTestRoot } from "@regenrek/gpuix-react/testing"
import type { DockLayout } from "@regenrek/gpuix-react"
import { QueryClientProvider } from "@tanstack/react-query"

import type { AgentRuntimeSnapshot, DesktopDaemonLifecycleProjection, RunEventDelta, RunStatus } from "@taugentic/desktop-protocol"
import type { NativeDaemonBridge } from "@taugentic/desktop-daemon-native"

import { commandRegistry, createCommandDispatcher } from "../src/features/commands/registry.js"
import { RuntimeRoutePicker } from "../src/features/auth-profiles/auth-profiles.js"
import { ProjectTrustConfirmation } from "../src/app/project-trust-confirmation.js"
import { App, Workbench, workbenchSelection } from "../src/app/App.js"
import { workspaceShellMachine } from "../src/features/runtime/workspace-shell-machine.js"
import { archiveConversation, closeTemporaryConversation, createProjectConversation, createSpace, createStandaloneConversation, createTemporaryConversation, desktopRuntime, openProject, selectConversation, setConversationPinned, setProjectSpace, startSelectedRun, triggerWorkItem, workspaceShell } from "../src/features/runtime/workspace-shell.js"
import { defaultWorkspaceLayout, resetWorkspaceLayout, workspacePresentation } from "../src/features/workspace-layout/layout-store.js"
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

  it("resets only the selected workspace layout and derives its default without persisting it", () => {
    const selectedWorkspaceId = "workspace-layout-reset-selected"
    const otherWorkspaceId = "workspace-layout-reset-other"
    const custom: DockLayout = { kind: "tabs", id: "custom", panels: ["terminal"], active: "terminal" }
    const originalAppearance = desktopSettings.appearance()
    const originalShortcut = desktopSettings.shortcut("focus-git")
    const appearance = { ...originalAppearance, theme: originalAppearance.theme === "dark" ? "light" as const : "dark" as const }
    const shortcut = "ctrl+shift+g"
    try {
      desktopSettings.saveAppearance(appearance)
      expect(desktopSettings.saveShortcut("focus-git", shortcut)).toBe("saved")
      desktopSettings.saveLayout(selectedWorkspaceId, custom)
      desktopSettings.saveLayout(otherWorkspaceId, custom)

      resetWorkspaceLayout(selectedWorkspaceId)

      expect(desktopSettings.presentation(selectedWorkspaceId)).toBeUndefined()
      expect(workspacePresentation(selectedWorkspaceId)).toEqual({ theme: appearance.theme, layout: defaultWorkspaceLayout })
      expect(desktopSettings.presentation(otherWorkspaceId)?.layout).toEqual(custom)
      expect(desktopSettings.appearance()).toEqual(appearance)
      expect(desktopSettings.shortcut("focus-git")).toBe(shortcut)
    } finally {
      desktopSettings.deleteLayout(selectedWorkspaceId)
      desktopSettings.deleteLayout(otherWorkspaceId)
      desktopSettings.saveAppearance(originalAppearance)
      desktopSettings.saveShortcut("focus-git", originalShortcut ?? "")
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
      expect(text).toContain("1. RUNTIME PROFILE")
      expect(text).toContain("2. CONNECTED ACCOUNT")
      expect(text).toContain("3. MODEL")
      expect(text).toContain("Personal · default · connected · Pro · weekly: 20/40")
      expect(text).toContain("Work · connected · Team · usage unavailable · rateLimited")
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

})
