import { DockWorkspace, useGpuixRequired } from "@regenrek/gpuix-react"
import { useInfiniteQuery, useQuery } from "@tanstack/react-query"
import { useCallback, useMemo, useRef, useState, useSyncExternalStore } from "react"
import type { NavigationSnapshot, ProjectId, RunId, SessionId } from "@taugentic/desktop-protocol"

import { CommandSurface } from "../features/commands/command-surface.js"
import { createCommandDispatcher, eventShortcut } from "../features/commands/registry.js"
import { RuntimeRoutePicker } from "../features/auth-profiles/auth-profiles.js"
import { useWorkbenchArtifacts } from "../features/artifacts/use-workbench-artifacts.js"
import { useApprovalsInbox } from "../features/approvals/use-approvals-inbox.js"
import { useWorkbenchCodeHost } from "../features/code-host/use-workbench-code-host.js"
import { useWorkbenchFiles } from "../features/files/use-workbench-files.js"
import { useWorkbenchGit } from "../features/git/use-workbench-git.js"
import { useRunActivity } from "../features/run-activity/use-run-activity.js"
import { useWorkbenchTerminal } from "../features/terminal/use-workbench-terminal.js"
import { useWorkbenchBrowser } from "../features/browser/use-workbench-browser.js"
import { useThreadWorkspace } from "../features/thread-workspace/use-thread-workspace.js"
import { WorkInboxPanel } from "../features/work-items/work-inbox-panel.js"
import { useWorkInbox } from "../features/work-items/use-work-inbox.js"
import { ScheduledWorkPanel } from "../features/scheduled-work/scheduled-work-panel.js"
import { useScheduledWork } from "../features/scheduled-work/use-scheduled-work.js"
import { PluginsPanel } from "../features/plugins/plugins-panel.js"
import { usePlugins } from "../features/plugins/use-plugins.js"
import { DiagnosticsPanel } from "../features/diagnostics/diagnostics-panel.js"
import { applySidebarAction, archiveConversation, cancelSelectedRun, cancelSideChat, closeSideChat, closeTemporaryConversation, closeWorkspaceShell, continueSideChat, createProjectConversation, createSpace, createStandaloneConversation, createTemporaryConversation, desktopRuntime, joinFreshRun, loginAuthMethod, logoutAuthProfile, openProject, openSideChat, openSideChatPanel, removeRunAttachment, replaceAuthProfilePreferences, requestVoicePermission, restoreConversation, retryWorkspaceShell, selectConversation, selectProject, setConversationPinned, setProjectSpace, spawnFreshRun, startSelectedRun, toggleRunAttachment, triggerWorkItem, updateRuntimeDraft, workspaceShell } from "../features/runtime/workspace-shell.js"
import { Sidebar } from "../features/sidebar/sidebar.js"
import { closeDockPanel, hasDockPanel, isDockPanelVisible, openDockPanel, resetWorkspaceLayout, saveDesktopTheme, saveWorkspaceLayout, workspacePresentation } from "../features/workspace-layout/layout-store.js"
import { panelRegistry } from "../features/workspace-layout/panels.js"
import { navigationQuery } from "../platform/daemon/navigation-query.js"
import { diagnosticsQuery } from "../platform/daemon/diagnostics-query.js"
import { recipesQuery } from "../platform/daemon/recipes-query.js"
import { transcriptQuery, transcriptRows } from "../platform/daemon/transcript-query.js"
import { conversationBranchesQuery, conversationBranchRows } from "../platform/daemon/conversation-branches-query.js"
import { desktopSettings } from "../platform/settings/desktop-settings.js"

import { applyDesktopAppearance, fontSize, metrics, palette } from "./theme.js"
import { ProductState } from "./product-state.js"
import { ProjectTrustConfirmation } from "./project-trust-confirmation.js"

function activates(event: { key?: string }): boolean { return event.key === "enter" || event.key === "space" }

function shellStatus(phase: "connecting" | "ready" | "unavailable" | "closed"): string {
  if (phase === "ready") return "DAEMON READY"
  if (phase === "connecting") return "CONNECTING DAEMON"
  if (phase === "closed") return "DAEMON CLOSED"
  return "DAEMON UNAVAILABLE"
}

export function OfflineConnectionState({ onRetry }: { onRetry(): void }) {
  return <div testId="daemon-offline" style={{ display: "flex", flexGrow: 1, alignItems: "center", justifyContent: "center" }}>
    <ProductState kind="offline" title="Daemon unavailable" detail="The connection is offline. Retry only when you are ready to reconnect." action={<div testId="retry-daemon" tabIndex={0} accessibilityRole="button" accessibilityName="Retry connection" onClick={onRetry} onKeyDown={(event) => { if (activates(event)) onRetry() }} style={{ cursor: "pointer", padding: 9, backgroundColor: palette.panelRaised }}><text>Retry connection</text></div>} />
  </div>
}

export function workbenchSelection(
  navigation: NavigationSnapshot,
  selectedProjectId?: ProjectId,
  selectedConversationId?: SessionId,
) {
  const browsingProject = navigation.projects?.find((project) => project.id === selectedProjectId)
  const conversation = navigation.conversations?.find((item) => item.sessionId === selectedConversationId)
  let project = browsingProject
  if (conversation) {
    const placement = conversation.placement
    project = placement.kind === "project"
      ? navigation.projects?.find((item) => item.id === placement.projectId)
      : undefined
  }
  return {
    browsingProject,
    conversation,
    project,
    workspaceId: conversation?.workspaceId ?? project?.workspaceIds?.[0],
  }
}

export function App() {
  const renderer = useGpuixRequired()
  const pendingProjectPath = useRef<string | null>(null)
  const [confirmingProjectTrust, setConfirmingProjectTrust] = useState(false)
  const [spaceTitle, setSpaceTitle] = useState("")
  const [conversationTitle, setConversationTitle] = useState("")
  const [standaloneTitle, setStandaloneTitle] = useState("")
  const [temporaryTitle, setTemporaryTitle] = useState("")
  const [selectedRecipeId, setSelectedRecipeId] = useState<string>()
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [pluginsOpen, setPluginsOpen] = useState(false)
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false)
  const settingsError = useSyncExternalStore(
    desktopSettings.subscribe,
    () => desktopSettings.error(),
  )
  const shell = useSyncExternalStore(
    (listener) => workspaceShell.subscribe(listener).unsubscribe,
    () => workspaceShell.getSnapshot(),
  )
  const snapshotQuery = useQuery({ ...navigationQuery(desktopRuntime), enabled: shell.context.phase === "ready" })
  const navigation = snapshotQuery.data ?? { spaces: [], projects: [], conversations: [], agents: [] }
  const searchActive = shell.context.sidebar.filter.length > 0
  const searchSnapshotQuery = useQuery({
    ...navigationQuery(desktopRuntime, shell.context.sidebar.filter),
    enabled: shell.context.phase === "ready" && searchActive,
  })
  const sidebarNavigation = searchActive
    ? searchSnapshotQuery.data ?? { spaces: [], projects: [], conversations: [], agents: [] }
    : navigation
  const selection = workbenchSelection(navigation, shell.context.selectedProjectId, shell.context.sidebar.selectedConversationId)
  const selectedProject = selection.browsingProject
  const selectedConversation = selection.conversation
  const workbenchProject = selection.project
  const selectedSessionId = shell.context.sidebar.selectedConversationId
  const selectedWorkspaceId = selection.workspaceId
  const selectedRuntimeProfile = shell.context.agentRuntime?.runtimeProfiles?.find((profile) => profile.id === shell.context.pendingSelection?.runtimeProfileId)
  const selectedProvider = shell.context.agentRuntime?.providers?.find((provider) => provider.id === selectedRuntimeProfile?.providerId)
  const selectedModel = selectedProvider?.models?.find((model) => model.id === shell.context.pendingSelection?.modelId)
  const replacementAuthProfileId = shell.context.agentRuntime?.authProfiles?.find((profile) => (
    profile.profile.id === shell.context.pendingSelection?.authProfileId
    && profile.connectionState === "connected"
    && profile.exhaustion == null
    && profile.profile.providerId === selectedRuntimeProfile?.providerId
    && profile.profile.authMethodId === selectedRuntimeProfile?.authMethodId
  ))?.profile.id
  const replacementSelection = replacementAuthProfileId
    && shell.context.pendingSelection?.runtimeProfileId
    && shell.context.pendingSelection?.modelId
    ? {
        runtimeProfileId: shell.context.pendingSelection.runtimeProfileId,
        authProfileId: replacementAuthProfileId,
        modelId: shell.context.pendingSelection.modelId,
      }
    : undefined
  const canStart = shell.context.phase === "ready" && !shell.context.activeRun && Boolean(shell.context.pendingSelection?.runtimeProfileId) && Boolean(shell.context.pendingSelection?.authProfileId) && Boolean(shell.context.pendingSelection?.modelId) && Boolean(shell.context.sidebar.selectedConversationId) && Boolean(shell.context.objective.trim())
  const canTriggerWorkItem = shell.context.phase === "ready" && !shell.context.activeRun && Boolean(shell.context.pendingSelection?.runtimeProfileId) && Boolean(shell.context.pendingSelection?.authProfileId) && Boolean(shell.context.pendingSelection?.modelId) && Boolean(shell.context.sidebar.selectedConversationId)
  const canCancel = shell.context.phase === "ready" && Boolean(shell.context.activeRun)
  const scheduledWorkSelection = shell.context.pendingSelection?.runtimeProfileId
    && shell.context.pendingSelection.authProfileId
    && shell.context.pendingSelection.modelId
    ? {
        runtimeProfileId: shell.context.pendingSelection.runtimeProfileId,
        authProfileId: shell.context.pendingSelection.authProfileId,
        modelId: shell.context.pendingSelection.modelId,
      }
    : undefined
  const chooseProjectDirectory = useCallback(async (): Promise<void> => {
    try {
      const path = await renderer.promptForDirectory()
      if (path === null) return
      pendingProjectPath.current = path
      setConfirmingProjectTrust(true)
    } catch {
      workspaceShell.send({ type: "ERROR", message: "The folder picker could not be opened." })
    }
  }, [renderer])
  const finishProjectTrust = async (acknowledged: boolean): Promise<void> => {
    const path = pendingProjectPath.current
    pendingProjectPath.current = null
    setConfirmingProjectTrust(false)
    if (!acknowledged || path === null) return
    await openProject(path, true)
  }
  const commands = useMemo(() => shell.context.phase === "ready" ? createCommandDispatcher(desktopSettings, () => ({ canStart, canCancel, hasWorkspace: Boolean(selectedWorkspaceId) }), {
    openSettings: () => setSettingsOpen(true),
    openProject: () => { void chooseProjectDirectory() },
    openDiagnostics: () => setDiagnosticsOpen(true),
    openPlugins: () => setPluginsOpen(true),
    openBrowser: () => saveWorkspaceLayout(selectedWorkspaceId!, openDockPanel(workspacePresentation(selectedWorkspaceId!).layout, "browser", "workspace-primary")),
    focusPanel: (panelId) => workspaceShell.send({ type: "FOCUS_PANEL", panelId }),
    toggleTheme: () => saveDesktopTheme(desktopSettings.appearance().theme === "dark" ? "light" : "dark"),
    startRun: () => void startSelectedRun(desktopRuntime, workspaceShell, selectedRecipeId),
    cancelRun: () => void cancelSelectedRun(),
  }) : undefined, [canCancel, canStart, chooseProjectDirectory, selectedRecipeId, selectedWorkspaceId, shell.context.phase])
  const recipeList = useQuery({ ...recipesQuery(desktopRuntime), enabled: shell.context.phase === "ready" })
  const diagnosticsSnapshot = useQuery({
    ...diagnosticsQuery(desktopRuntime),
    enabled: diagnosticsOpen && shell.context.phase === "ready",
  })
  const diagnosticsState = shell.context.phase !== "ready"
    ? "unavailable" as const
    : diagnosticsSnapshot.isPending || diagnosticsSnapshot.isFetching
      ? "loading" as const
      : diagnosticsSnapshot.isError
        ? "error" as const
        : "ready" as const
  const plugins = usePlugins({ runtime: desktopRuntime, enabled: shell.context.phase === "ready" })
  const files = useWorkbenchFiles({
    runtime: desktopRuntime,
    projectId: workbenchProject?.id,
    workspaceId: selectedWorkspaceId,
    enabled: shell.context.phase === "ready",
  })
  const artifacts = useWorkbenchArtifacts({
    runtime: desktopRuntime,
    sessionId: selectedSessionId,
    enabled: shell.context.phase === "ready",
  })
  const approvalsInbox = useApprovalsInbox({
    runtime: desktopRuntime,
    sessionId: selectedSessionId,
    enabled: shell.context.phase === "ready",
  })
  const browser = useWorkbenchBrowser(desktopRuntime, shell.context.phase === "ready")
  const runActivity = useRunActivity({
    runtime: desktopRuntime,
    sessionId: selectedSessionId,
    replacementSelection,
    enabled: shell.context.phase === "ready",
    approvals: approvalsInbox.approvals,
    decideApproval: approvalsInbox.decide,
    openArtifact: (artifactId) => {
      artifacts.selectArtifact(artifactId)
      workspaceShell.send({ type: "FOCUS_PANEL", panelId: "image" })
    },
  })
  const terminal = useWorkbenchTerminal({
    runtime: desktopRuntime,
    renderer,
    projectId: workbenchProject?.id,
    workspaceId: selectedWorkspaceId,
    enabled: shell.context.phase === "ready",
  })
  const git = useWorkbenchGit({
    runtime: desktopRuntime,
    projectId: workbenchProject?.id,
    workspaceId: selectedWorkspaceId,
    enabled: shell.context.phase === "ready",
    runStatus: shell.context.runStatus,
  })
  const codeHost = useWorkbenchCodeHost({
    runtime: desktopRuntime,
    projectId: workbenchProject?.id,
    workspaceId: selectedWorkspaceId,
    enabled: shell.context.phase === "ready",
    branch: git.snapshot?.branch ?? undefined,
    runActive: Boolean(shell.context.activeRun),
  })
  const transcriptPageQuery = useInfiniteQuery({
    ...transcriptQuery(desktopRuntime, selectedSessionId ?? "session-not-selected"),
    enabled: shell.context.phase === "ready" && Boolean(selectedSessionId),
  })
  const durableTranscriptRows = transcriptRows(transcriptPageQuery.data?.pages)
  const branchQuery = useQuery({
    ...conversationBranchesQuery(desktopRuntime, selectedSessionId ?? "session-not-selected"),
    enabled: shell.context.phase === "ready" && Boolean(selectedSessionId),
  })
  const branches = conversationBranchRows(branchQuery.data)
  const visibleSideChats = branches.filter((branch) => branch.relationship.kind === "fork" && !shell.context.closedSideChatIds.includes(branch.id))
  const presentation = useWorkspacePresentation(selectedWorkspaceId)
  applyDesktopAppearance(desktopSettings.appearance())
  const threadWorkspace = useThreadWorkspace({
    runtime: desktopRuntime,
    sessionId: selectedSessionId,
    enabled: shell.context.phase === "ready",
  })
  const workInbox = useWorkInbox({
    runtime: desktopRuntime,
    enabled: shell.context.phase === "ready",
    canTrigger: canTriggerWorkItem,
    trigger: triggerWorkItem,
  })
  const scheduledWork = useScheduledWork({
    runtime: desktopRuntime,
    enabled: shell.context.phase === "ready" && Boolean(selectedSessionId),
    sessionId: selectedSessionId,
    selection: scheduledWorkSelection,
  })
  const selectedAttachmentRevision = files.selectedContent?.revision ?? files.selectedImagePreview?.revision
  const filePanelState = {
    ...files,
    attached: Boolean(files.selectedPath && shell.context.attachments.some((attachment) => attachment.path === files.selectedPath)),
    attachmentEnabled: files.selectedEntry?.kind !== "image" || selectedModel?.mediaCapabilities.imageInput === "supported",
    toggleAttachment: () => {
      if (!files.selectedPath || !selectedAttachmentRevision) return
      toggleRunAttachment({ path: files.selectedPath, expectedRevision: selectedAttachmentRevision })
    },
  }
  const artifactPanelState = {
    ...artifacts,
    openImageArtifact: (artifactId: import("@taugentic/desktop-protocol").ArtifactId) => {
      artifacts.selectArtifact(artifactId)
      workspaceShell.send({ type: "FOCUS_PANEL", panelId: "image" })
    },
  }
  const panels = panelRegistry({
    title: selectedConversation?.title ?? "Select a conversation",
    selectedConversationId: shell.context.sidebar.selectedConversationId,
    transcriptRows: durableTranscriptRows,
    transcriptLoading: transcriptPageQuery.isLoading,
    transcriptError: transcriptPageQuery.isError ? "The conversation transcript could not be loaded." : undefined,
    hasOlderTranscript: Boolean(transcriptPageQuery.hasNextPage),
    loadingOlderTranscript: transcriptPageQuery.isFetchingNextPage,
    onLoadOlderTranscript: () => { void transcriptPageQuery.fetchNextPage() },
    messages: shell.context.messages,
    objective: shell.context.objective,
    attachments: shell.context.attachments,
    error: shell.context.error ?? shell.context.navigationError,
    runStatus: shell.context.runStatus,
    voice: {
      visible: selectedRuntimeProfile?.executionKind === "realtimeVoice",
      permission: shell.context.voicePermission,
      state: shell.context.voice,
      runStatus: shell.context.runStatus,
      onRequestPermission: requestVoicePermission,
    },
    recipes: recipeList.data?.recipes ?? [],
    recipesLoading: recipeList.isLoading,
    recipesError: recipeList.isError ? "Recipes could not be loaded." : undefined,
    selectedRecipeId,
    onSelectRecipe: setSelectedRecipeId,
    onObjectiveChange: (objective) => workspaceShell.send({ type: "SET_OBJECTIVE", objective }),
    onRemoveAttachment: removeRunAttachment,
    commands: commands!,
    branches,
    branchGraph: branchQuery.data,
    branchGraphState: branchQuery.isLoading ? "loading" : shell.context.phase !== "ready" ? "offline" : branchQuery.isError ? "error" : "ready",
    cortexVisible: isDockPanelVisible(presentation.layout, "conversation"),
    sideChats: visibleSideChats,
    onOpenSideChat: (parentRunId, parentEventSeq) => void openSideChat(parentRunId, parentEventSeq),
    onCancelSideChat: (runId) => void cancelSideChat(runId),
    onCloseSideChat: closeSideChat,
    onContinueSideChat: (runId, message) => void continueSideChat(runId, message),
    onSpawnFresh: spawnFreshRun,
    onJoinFresh: joinFreshRun,
    onOpenSideChatPanel: openSideChatPanel,
    onPinThreadWorkspace: threadWorkspace.addPin,
    files: filePanelState,
    artifacts: artifactPanelState,
    terminal,
    git,
    codeHost,
    threadWorkspace,
    runActivity,
    approvalsInbox,
    browser: hasDockPanel(presentation.layout, "browser") ? browser : undefined,
    browserVisible: isDockPanelVisible(presentation.layout, "browser"),
    closeBrowser: () => {
      if (selectedWorkspaceId) saveWorkspaceLayout(selectedWorkspaceId, closeDockPanel(presentation.layout, "browser"))
    },
    openUrl: (url) => renderer.openUrl(url),
    copyText: (text) => renderer.writeClipboardText(text),
  })
  return <div style={{ display: "flex", flexDirection: "column", width: "100%", height: "100%", backgroundColor: palette.canvas, color: palette.text }}>
    <div style={{ display: "flex", alignItems: "center", height: metrics.titlebarHeight, paddingLeft: 86, paddingRight: 18, userSelect: "none" }}>
      <text style={{ color: palette.text, fontSize: fontSize(13), fontWeight: 650 }}>TAUGENTIC</text><div style={{ flexGrow: 1 }} />
      {shell.context.phase === "ready" && <><div testId="open-system-diagnostics" tabIndex={0} accessibilityRole="button" accessibilityName="Open System Diagnostics" onClick={() => commands!.dispatch("open-diagnostics")} onKeyDown={(event) => { if (activates(event)) commands!.dispatch("open-diagnostics") }} style={{ cursor: "pointer", padding: 7, backgroundColor: palette.panelRaised, marginRight: 8 }}><text>Diagnostics</text></div>
      <div testId="open-plugins" tabIndex={0} accessibilityRole="button" accessibilityName="Open Plugins" onClick={() => commands!.dispatch("open-plugins")} onKeyDown={(event) => { if (activates(event)) commands!.dispatch("open-plugins") }} style={{ cursor: "pointer", padding: 7, backgroundColor: palette.panelRaised, marginRight: 8 }}><text>Plugins</text></div>
      <CommandSurface dispatcher={commands!} settings={desktopSettings} workspaceId={selectedWorkspaceId} settingsOpen={settingsOpen} onSettingsOpenChange={setSettingsOpen} onResetWorkspaceLayout={() => { if (selectedWorkspaceId) resetWorkspaceLayout(selectedWorkspaceId) }} /></>}
      <text testId="daemon-status" style={{ color: shell.context.phase === "ready" ? palette.accent : shell.context.phase === "unavailable" ? palette.warning : palette.textMuted, fontSize: fontSize(11) }}>{shellStatus(shell.context.phase)}</text>
    </div>
    <div style={{ height: 1, backgroundColor: palette.border }} />
    {settingsError && <div testId="desktop-settings-error" accessibilityRole="alert" accessibilityName={settingsError} style={{ padding: 8, backgroundColor: "#401c24", color: "#f08080" }}><text>{settingsError}</text></div>}
    {shell.context.phase === "unavailable" ? <OfflineConnectionState onRetry={() => void retryWorkspaceShell()} /> : shell.context.phase === "ready" ? <><RuntimeRoutePicker snapshot={shell.context.agentRuntime} draft={shell.context.pendingSelection} pendingAuthMethodIds={shell.context.pendingAuthMethodIds} onDraft={(draft) => void updateRuntimeDraft(draft)} onLogin={(id) => void loginAuthMethod(id, (authorizeUrl) => renderer.openUrl(authorizeUrl))} onLogout={(id) => void logoutAuthProfile(id)} onPreferences={(id, preferences) => void replaceAuthProfilePreferences(id, preferences)} />
    <div testId="workspace-shell" style={{ display: "flex", flexGrow: 1, minHeight: 0, width: "100%", height: "100%" }}>
      <Sidebar snapshot={sidebarNavigation} state={shell.context.sidebar} selectedProjectId={shell.context.selectedProjectId} spaceTitle={spaceTitle} conversationTitle={conversationTitle} standaloneTitle={standaloneTitle} temporaryTitle={temporaryTitle} canCreateSpace={shell.context.phase === "ready" && !shell.context.activeRun && Boolean(spaceTitle.trim())} canCreateConversation={Boolean(selectedProject?.workspaceIds?.[0]) && Boolean(conversationTitle.trim())} canCreateStandalone={shell.context.phase === "ready" && !shell.context.activeRun && Boolean(selectedWorkspaceId) && Boolean(standaloneTitle.trim())} canCreateTemporary={shell.context.phase === "ready" && !shell.context.activeRun && Boolean(selectedWorkspaceId) && Boolean(temporaryTitle.trim())} canOrganizeConversations={shell.context.phase === "ready" && !shell.context.activeRun} canOrganizeProjects={shell.context.phase === "ready" && !shell.context.activeRun} searchMode={searchActive} searchLoading={searchSnapshotQuery.isLoading} searchError={searchSnapshotQuery.isError} workInbox={<WorkInboxPanel inbox={workInbox} canTrigger={canTriggerWorkItem} />} scheduledWork={<ScheduledWorkPanel scheduledWork={scheduledWork} onOpenRun={(runId) => { runActivity.selectRun(runId as RunId); workspaceShell.send({ type: "FOCUS_PANEL", panelId: "activity" }) }} />} onSpaceTitleChange={setSpaceTitle} onCreateSpace={() => { void createSpace(spaceTitle).then((created) => { if (created) setSpaceTitle("") }) }} onSetProjectSpace={(projectId, spaceId) => void setProjectSpace(projectId, spaceId)} onConversationTitleChange={setConversationTitle} onStandaloneTitleChange={setStandaloneTitle} onTemporaryTitleChange={setTemporaryTitle} onSetPinnedConversation={(sessionId, pinned) => void setConversationPinned(sessionId, pinned)} onArchiveConversation={(sessionId) => void archiveConversation(sessionId)} onRestoreConversation={(sessionId) => void restoreConversation(sessionId)} onCloseTemporaryConversation={(sessionId) => void closeTemporaryConversation(sessionId)} onOpenAttention={(sessionId) => void selectConversation(sessionId, true, "activity")} onCreateConversation={() => {
        const workspaceId = selectedProject?.workspaceIds?.[0]
        if (!selectedProject || !workspaceId || !conversationTitle.trim()) return
        void createProjectConversation(selectedProject.id, workspaceId, conversationTitle).then((created) => {
          if (created) setConversationTitle("")
        })
      }} onCreateStandalone={() => {
        if (!selectedWorkspaceId || !standaloneTitle.trim()) return
        void createStandaloneConversation(selectedWorkspaceId, standaloneTitle).then((created) => {
          if (created) setStandaloneTitle("")
        })
      }} onCreateTemporary={() => {
        if (!selectedWorkspaceId || !temporaryTitle.trim()) return
        void createTemporaryConversation(selectedWorkspaceId, temporaryTitle).then((created) => {
          if (created) setTemporaryTitle("")
        })
      }} dispatch={(action) => {
        applySidebarAction(action)
        if (action.type === "selectConversation") void selectConversation(action.sessionId)
        if (action.type === "selectProject") selectProject(action.projectId)
        if (action.type === "openProject") commands!.dispatch("open-project")
      }} />
      <div style={{ flexGrow: 1, minWidth: 0, minHeight: 0 }}>
        {selectedWorkspaceId && commands
          ? <Workbench workspaceId={selectedWorkspaceId} presentation={presentation} panels={panels} focusPanelId={shell.context.focusPanelId} commands={commands} />
          : <div testId="workspace-awaiting-project" style={{ display: "flex", flexDirection: "column", gap: 10, alignItems: "center", justifyContent: "center", height: "100%" }}><ProductState kind="empty" title="Select a project to open the workbench." action={<div testId="open-project" tabIndex={0} accessibilityRole="button" accessibilityName="Open project" onClick={() => commands!.dispatch("open-project")} onKeyDown={(event) => { if (activates(event)) commands!.dispatch("open-project") }} style={{ cursor: "pointer", padding: 9, backgroundColor: palette.panelRaised }}><text>Open project</text></div>} /></div>}
      </div>
    </div>
    {confirmingProjectTrust && <ProjectTrustConfirmation onDecision={(acknowledged) => void finishProjectTrust(acknowledged)} />}
    {pluginsOpen && <PluginsPanel plugins={plugins} onClose={() => setPluginsOpen(false)} />}
    {diagnosticsOpen && <DiagnosticsPanel state={diagnosticsState} diagnostics={diagnosticsSnapshot.data} onClose={() => setDiagnosticsOpen(false)} />}
    <div style={{ position: "absolute", right: 18, bottom: 18, display: "flex", gap: 8 }}>
      <div testId="close-daemon" tabIndex={0} accessibilityRole="button" accessibilityName="Close connection" onClick={() => void closeWorkspaceShell()} onKeyDown={(event) => { if (activates(event)) void closeWorkspaceShell() }} style={{ padding: 8, backgroundColor: palette.panelRaised, cursor: "pointer" }}><text>Close connection</text></div>
    </div>
  </> : <div testId="daemon-connecting" style={{ display: "flex", flexGrow: 1, alignItems: "center", justifyContent: "center" }}><ProductState kind={shell.context.phase === "closed" ? "offline" : "loading"} title={shell.context.phase === "closed" ? "Connection closed" : "Connecting daemon"} detail={shell.context.phase === "closed" ? "Reopen the desktop application to create a new connection." : "Waiting for the daemon lifecycle."} /></div>}
  </div>
}

export function Workbench({ workspaceId, presentation, panels, focusPanelId, commands }: { workspaceId: import("@taugentic/desktop-protocol").WorkspaceId; presentation: ReturnType<typeof workspacePresentation>; panels: ReturnType<typeof panelRegistry>; focusPanelId?: import("../features/commands/registry.js").FocusablePanelId; commands: import("../features/commands/registry.js").CommandDispatcher }) {
  return <DockWorkspace testId="workspace-dock" layout={presentation.layout} panels={panels} focusPanelId={focusPanelId} onLayoutChange={(layout) => saveWorkspaceLayout(workspaceId, layout)} onKeyDown={(event) => {
    const command = commands.commandForShortcut(eventShortcut(event))
    if (command) commands.dispatch(command)
  }} style={{ width: "100%", height: "100%" }} accessibilityName="Taugentic workspace" />
}

function useWorkspacePresentation(workspaceId?: import("@taugentic/desktop-protocol").WorkspaceId) {
  const revision = useSyncExternalStore(desktopSettings.subscribe, desktopSettings.revision)
  return useMemo(() => workspacePresentation(workspaceId), [revision, workspaceId])
}
