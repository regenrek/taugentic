import { createActor } from "xstate"
import { describe, expect, it } from "bun:test"
import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { QueryClientProvider } from "@tanstack/react-query"

import type { AgentRuntimeSnapshot, DesktopDaemonLifecycleProjection, NavigationSnapshot, RunEventDelta, RunStatus } from "@taugentic/desktop-protocol"
import type { NativeDaemonBridge } from "@taugentic/desktop-daemon-native"

import { commandRegistry, createCommandDispatcher } from "../src/features/commands/registry.js"
import { RuntimeRoutePicker } from "../src/features/auth-profiles/auth-profiles.js"
import { ProjectTrustConfirmation } from "../src/app/project-trust-confirmation.js"
import { App, Workbench, workbenchSelection } from "../src/app/App.js"
import { workspaceShellMachine } from "../src/features/runtime/workspace-shell-machine.js"
import { archiveConversation, closeTemporaryConversation, createProjectConversation, createSpace, createStandaloneConversation, createTemporaryConversation, createWorkspaceNavigationRecovery, desktopRuntime, openProject, selectConversation, setConversationPinned, setProjectSpace, startSelectedRun, triggerWorkItem, workspaceShell } from "../src/features/runtime/workspace-shell.js"
import { defaultWorkspaceLayout, workspacePresentation } from "../src/features/workspace-layout/layout-store.js"
import { archivedProjectConversations, projectConversations, sidebarReduce, Sidebar, standaloneConversations, temporaryConversations, type SidebarState } from "../src/features/sidebar/sidebar.js"
import { createDesktopRuntime } from "../src/platform/daemon/desktop-runtime.js"
import { navigationQuery, navigationQueryKey } from "../src/platform/daemon/navigation-query.js"
import { desktopQueryClient } from "../src/platform/daemon/query-client.js"
import { runActivityQueryRoot } from "../src/platform/daemon/run-activity-query.js"
import { scheduledWorkQueryKey } from "../src/platform/daemon/scheduled-work-query.js"
import { DesktopSettings, desktopSettings } from "../src/platform/settings/desktop-settings.js"

function lifecycle(
  status: DesktopDaemonLifecycleProjection["status"],
  invalidated = status !== "ready",
): DesktopDaemonLifecycleProjection {
  return { status, invalidated, foreignRuntimeRestricted: false }
}

function navigationSnapshot(...sessionIds: string[]): NavigationSnapshot {
  return {
    spaces: [],
    projects: [],
    agents: [],
    conversations: sessionIds.map((sessionId) => ({
      sessionId,
      workspaceId: `workspace-${sessionId}`,
      title: sessionId,
      status: "idle" as const,
      attention: { pendingApproval: false, scheduledWorkRequiresAction: false },
      placement: { kind: "standalone" as const },
      archived: false,
      pinned: false,
    })),
  }
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

describe("M2 navigation concurrency", () => {
  it("treats sidebar conversation selection as an attach intent until the real App dispatch succeeds", async () => {
    const bridge = desktopRuntime.bridge as unknown as { attachSession(sessionId: string): Promise<string> }
    const originalAttach = bridge.attachSession
    let resolveAttach!: (value: string) => void
    const snapshot = {
      spaces: [], agents: [],
      projects: [{ id: "project-a", title: "Project A", workspaceIds: ["workspace-a"] }],
      conversations: [
        { sessionId: "old", title: "Old", status: "idle" as const, attention: { pendingApproval: false, scheduledWorkRequiresAction: false }, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: false },
        { sessionId: "target", title: "Target", status: "idle" as const, attention: { pendingApproval: true, scheduledWorkRequiresAction: true }, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: false },
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
      const entry = root.renderer.findByTestId("conversation-attention-target")!
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
      expect(workspaceShell.getSnapshot().context.focusPanelId).toBe("activity")
      expect(root.renderer.getAutomationTree()).toContain('"testId":"conversation-attention-target","accessibility":{"role":"button","name":"Open Activity for Target: pending approval; scheduled work requires action"')
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
    const baseline = { spaces: [], projects: [], agents: [], conversations: [{ sessionId: "baseline", title: "Baseline", status: "idle", attention: { pendingApproval: false, scheduledWorkRequiresAction: false }, placement: { kind: "standalone" }, archived: false, pinned: false }] }
    workspaceShell.start()
    try {
      desktopQueryClient.setQueryData(navigationQueryKey, baseline)
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      bridge.navigationIntent = () => new Promise<string>((resolve) => { resolveIntent = resolve })
      bridge.openProject = () => new Promise<string>((resolve) => { resolveOpen = resolve })

      const pin = setConversationPinned("conversation-stale", true)
      const open = openProject("/project", true)
      resolveIntent(JSON.stringify({ spaces: [], projects: [], agents: [], conversations: [{ sessionId: "stale", title: "Stale", status: "idle", attention: { pendingApproval: false, scheduledWorkRequiresAction: false }, placement: { kind: "standalone" }, archived: false, pinned: true }] }))
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
      navigationSnapshot(): Promise<string>
    }
    const originalIntent = bridge.navigationIntent
    const originalOpenSession = bridge.openSession
    const originalNavigationSnapshot = bridge.navigationSnapshot
    let resolveIntent!: (value: string) => void
    let resolveSession!: (value: string) => void
    const baseline = { spaces: [], projects: [], agents: [], conversations: [{ sessionId: "baseline", title: "Baseline", status: "idle", attention: { pendingApproval: false, scheduledWorkRequiresAction: false }, placement: { kind: "standalone" }, archived: false, pinned: false }] }
    workspaceShell.start()
    try {
      desktopQueryClient.setQueryData(navigationQueryKey, baseline)
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      bridge.navigationIntent = () => new Promise<string>((resolve) => { resolveIntent = resolve })
      bridge.openSession = () => new Promise<string>((resolve) => { resolveSession = resolve })
      bridge.navigationSnapshot = () => Promise.resolve(JSON.stringify(navigationSnapshot("created")))

      const pin = setConversationPinned("conversation-stale", true)
      const create = createProjectConversation("project-a", "workspace-a", "Created")
      resolveIntent(JSON.stringify({ spaces: [], projects: [], agents: [], conversations: [{ sessionId: "stale", title: "Stale", status: "idle", attention: { pendingApproval: false, scheduledWorkRequiresAction: false }, placement: { kind: "standalone" }, archived: false, pinned: true }] }))
      await pin
      expect(desktopQueryClient.getQueryData<unknown>(navigationQueryKey)).toEqual(baseline)
      resolveSession(JSON.stringify({ id: "created", nextRunSelection: { kind: "none" } }))
      await create
    } finally {
      bridge.navigationIntent = originalIntent
      bridge.openSession = originalOpenSession
      bridge.navigationSnapshot = originalNavigationSnapshot
      workspaceShell.stop()
    }
  })

  it("publishes the authoritative navigation snapshot before selecting a created conversation", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      openSession(paramsJson: string): Promise<string>
      navigationSnapshot(): Promise<string>
    }
    const originalOpenSession = bridge.openSession
    const originalNavigationSnapshot = bridge.navigationSnapshot
    let resolveNavigation!: (value: string) => void
    let markNavigationRequested!: () => void
    const navigationRequested = new Promise<void>((resolve) => { markNavigationRequested = resolve })
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "existing" })
      bridge.openSession = () => Promise.resolve(JSON.stringify({ id: "created", nextRunSelection: { kind: "none" } }))
      bridge.navigationSnapshot = () => {
        markNavigationRequested()
        return new Promise<string>((resolve) => { resolveNavigation = resolve })
      }

      const creation = createProjectConversation("project-a", "workspace-a", "Created")
      await navigationRequested
      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("existing")

      const authoritativeSnapshot = navigationSnapshot("existing", "created")
      resolveNavigation(JSON.stringify(authoritativeSnapshot))
      expect(await creation).toBe(true)
      expect(desktopQueryClient.getQueryData<NavigationSnapshot>(navigationQueryKey)).toEqual(authoritativeSnapshot)
      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("created")
    } finally {
      bridge.openSession = originalOpenSession
      bridge.navigationSnapshot = originalNavigationSnapshot
      workspaceShell.stop()
    }
  })

  it("preserves the selected conversation when the authoritative create refresh fails", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      openSession(paramsJson: string): Promise<string>
      navigationSnapshot(): Promise<string>
    }
    const originalOpenSession = bridge.openSession
    const originalNavigationSnapshot = bridge.navigationSnapshot
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      workspaceShell.send({ type: "SELECTED", sessionId: "existing" })
      bridge.openSession = () => Promise.resolve(JSON.stringify({ id: "created", nextRunSelection: { kind: "none" } }))
      bridge.navigationSnapshot = () => Promise.reject(new Error("navigation unavailable"))

      expect(await createProjectConversation("project-a", "workspace-a", "Created")).toBe(false)
      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("existing")
      expect(workspaceShell.getSnapshot().context.error).toBe(
        "The conversation could not be created or refreshed. Refresh navigation and try again.",
      )
    } finally {
      bridge.openSession = originalOpenSession
      bridge.navigationSnapshot = originalNavigationSnapshot
      workspaceShell.stop()
    }
  })

  it("commits only the newest concurrent project conversation creation", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      openSession(paramsJson: string): Promise<string>
      navigationSnapshot(): Promise<string>
    }
    const originalOpenSession = bridge.openSession
    const originalNavigationSnapshot = bridge.navigationSnapshot
    let resolveFirst!: (value: string) => void
    let resolveSecond!: (value: string) => void
    workspaceShell.start()
    try {
      workspaceShell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      bridge.openSession = (paramsJson) => new Promise<string>((resolve) => {
        if (JSON.parse(paramsJson).title === "First") resolveFirst = resolve
        else resolveSecond = resolve
      })
      bridge.navigationSnapshot = () => Promise.resolve(JSON.stringify(navigationSnapshot("second")))

      const first = createProjectConversation("project-a", "workspace-a", "First")
      const second = createProjectConversation("project-a", "workspace-a", "Second")
      resolveSecond(JSON.stringify({ id: "second", nextRunSelection: { kind: "none" } }))
      expect(await second).toBe(true)
      resolveFirst(JSON.stringify({ id: "first", nextRunSelection: { kind: "none" } }))
      expect(await first).toBe(false)

      expect(workspaceShell.getSnapshot().context.sidebar.selectedConversationId).toBe("second")
    } finally {
      bridge.openSession = originalOpenSession
      bridge.navigationSnapshot = originalNavigationSnapshot
      workspaceShell.stop()
    }
  })

  it("opens standalone conversations by workspace id and keeps only the current temporary creation selected", async () => {
    const bridge = desktopRuntime.bridge as unknown as {
      openSession(paramsJson: string): Promise<string>
      navigationSnapshot(): Promise<string>
    }
    const originalOpenSession = bridge.openSession
    const originalNavigationSnapshot = bridge.navigationSnapshot
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
      bridge.navigationSnapshot = () => Promise.resolve(JSON.stringify(navigationSnapshot("standalone", "temporary-second")))

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
      bridge.navigationSnapshot = originalNavigationSnapshot
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
      attention: { pendingApproval: false, scheduledWorkRequiresAction: false },
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

  it("round-trips only current-shape navigation settings and rejects the old shape", async () => {
    const writes: string[] = []
    const document = {
      appearance: { theme: "dark", contrast: "standard", fontScale: "standard", reducedMotion: false },
      layouts: {},
      shortcuts: {},
      navigation: { sidebarView: "projects", expandedSpaceIds: ["space-a"], selectedProjectId: "project-a", selectedSessionId: "session-a" },
    } as const
    const settings = new DesktopSettings()
    await settings.initialize({ read: async () => JSON.stringify(document), write: async (value) => { writes.push(value) } })
    expect(settings.navigation()).toEqual(document.navigation)
    settings.saveNavigation({ sidebarView: "archived", expandedSpaceIds: [], selectedProjectId: "project-b" })
    await Promise.resolve()
    expect(JSON.parse(writes[0] ?? "{}").navigation).toEqual({ sidebarView: "archived", expandedSpaceIds: [], selectedProjectId: "project-b" })
    expect(JSON.parse(writes[0] ?? "{}").navigation).not.toHaveProperty("filter")

    const oldShape = new DesktopSettings()
    await oldShape.initialize({ read: async () => JSON.stringify({ appearance: document.appearance, layouts: {}, shortcuts: {} }), write: async () => {} })
    expect(oldShape.error()).toBeDefined()
    expect(oldShape.navigation()).toEqual({ sidebarView: "spaces", expandedSpaceIds: [] })
  })

  it("restores valid saved navigation once through the existing attach route", async () => {
    const shell = createActor(workspaceShellMachine).start()
    const settings = new DesktopSettings()
    const snapshot = {
      spaces: [{ id: "space-a", title: "Space A" }],
      agents: [],
      projects: [{ id: "project-a", title: "Project A", workspaceIds: ["workspace-a"] }],
      conversations: [{ sessionId: "session-a", workspaceId: "workspace-a", title: "Session A", status: "idle" as const, attention: { pendingApproval: false, scheduledWorkRequiresAction: false }, placement: { kind: "project" as const, projectId: "project-a" }, archived: false, pinned: false }],
    }
    const attached: string[] = []
    settings.saveNavigation({ sidebarView: "projects", expandedSpaceIds: ["space-a"], selectedProjectId: "project-a", selectedSessionId: "session-a" })
    const recovery = createWorkspaceNavigationRecovery(shell, settings, async (sessionId) => {
      attached.push(sessionId)
      shell.send({ type: "SELECTED", sessionId })
    })
    try {
      shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      recovery.applyStoredSidebarPresentation()
      await recovery.restoreOnce(snapshot)
      expect(shell.getSnapshot().context.selectedProjectId).toBe("project-a")
      expect(shell.getSnapshot().context.sidebar.selectedConversationId).toBe("session-a")
      expect(attached).toEqual(["session-a"])
      await recovery.restoreOnce(snapshot)
      expect(attached).toEqual(["session-a"])
    } finally {
      shell.stop()
    }
  })

  it("leaves invalid saved identities unselected without a replacement attach", async () => {
    const shell = createActor(workspaceShellMachine).start()
    const settings = new DesktopSettings()
    const attached: string[] = []
    settings.saveNavigation({ sidebarView: "spaces", expandedSpaceIds: [], selectedProjectId: "absent-project", selectedSessionId: "archived-session" })
    const recovery = createWorkspaceNavigationRecovery(shell, settings, async (sessionId) => { attached.push(sessionId) })
    try {
      shell.send({ type: "LIFECYCLE", projection: lifecycle("ready", false) })
      await recovery.restoreOnce({ spaces: [], agents: [], projects: [{ id: "present-project", title: "Present", workspaceIds: ["workspace-present"] }], conversations: [{ sessionId: "archived-session", workspaceId: "workspace-present", title: "Archived", status: "idle" as const, attention: { pendingApproval: false, scheduledWorkRequiresAction: false }, placement: { kind: "project" as const, projectId: "present-project" }, archived: true, pinned: false }] })
      expect(shell.getSnapshot().context.selectedProjectId).toBeUndefined()
      expect(shell.getSnapshot().context.sidebar.selectedConversationId).toBeUndefined()
      expect(attached).toEqual([])
    } finally { shell.stop() }
  })

})
