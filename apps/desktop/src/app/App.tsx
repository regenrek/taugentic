import { DockWorkspace, useGpuixRequired } from "@gpuix/react"
import { useQuery } from "@tanstack/react-query"
import { useRef, useState, useSyncExternalStore } from "react"

import { commandRegistry } from "../features/commands/registry.js"
import { RuntimeRoutePicker } from "../features/auth-profiles/auth-profiles.js"
import { cancelSelectedRun, closeWorkspaceShell, createProjectConversation, decideApproval, desktopRuntime, loginAuthMethod, logoutAuthProfile, openProject, selectConversation, selectProject, startSelectedRun, updateRuntimeDraft, workspaceShell } from "../features/runtime/workspace-shell-machine.js"
import { Sidebar } from "../features/sidebar/sidebar.js"
import { saveWorkspaceLayout, workspacePresentation } from "../features/workspace-layout/layout-store.js"
import { panelRegistry } from "../features/workspace-layout/panels.js"
import { navigationQuery } from "../platform/daemon/navigation-query.js"
import { desktopSettings } from "../platform/settings/desktop-settings.js"

import { metrics, palette } from "./theme.js"

function shellStatus(phase: "connecting" | "ready" | "unavailable" | "closed"): string {
  if (phase === "ready") return "DAEMON READY"
  if (phase === "connecting") return "CONNECTING DAEMON"
  if (phase === "closed") return "DAEMON CLOSED"
  return "DAEMON UNAVAILABLE"
}

export function App() {
  const renderer = useGpuixRequired()
  const pendingProjectPath = useRef<string | null>(null)
  const [confirmingProjectTrust, setConfirmingProjectTrust] = useState(false)
  const [conversationTitle, setConversationTitle] = useState("")
  const shell = useSyncExternalStore(
    (listener) => workspaceShell.subscribe(listener).unsubscribe,
    () => workspaceShell.getSnapshot(),
  )
  const snapshotQuery = useQuery({ ...navigationQuery(desktopRuntime), enabled: shell.context.phase === "ready" })
  const navigation = snapshotQuery.data ?? { spaces: [], projects: [], conversations: [], agents: [] }
  const selectedProject = navigation.projects?.find((project) => project.id === shell.context.selectedProjectId)
  const selectedConversation = navigation.conversations?.find((conversation) => (
    conversation.sessionId === shell.context.sidebar.selectedConversationId
    && conversation.placement.kind === "project"
    && conversation.placement.projectId === selectedProject?.id
  ))
  const panels = panelRegistry({
    title: selectedConversation?.title ?? "Select a conversation",
    selectedConversationId: shell.context.sidebar.selectedConversationId,
    messages: shell.context.messages,
    approvals: shell.context.approvals,
    objective: shell.context.objective,
    error: shell.context.error ?? shell.context.navigationError,
    canStart: shell.context.phase === "ready" && !shell.context.activeRun && Boolean(shell.context.runtimeDraft?.runtimeProfileId) && Boolean(shell.context.runtimeDraft?.authProfileId) && Boolean(shell.context.runtimeDraft?.modelId) && Boolean(shell.context.sidebar.selectedConversationId) && Boolean(shell.context.objective.trim()),
    canCancel: shell.context.phase === "ready" && Boolean(shell.context.activeRun),
    runStatus: shell.context.runStatus,
    onObjectiveChange: (objective) => workspaceShell.send({ type: "SET_OBJECTIVE", objective }),
    onStart: () => void startSelectedRun(),
    onCancel: () => void cancelSelectedRun(),
    onDecideApproval: (approvalId, decision) => void decideApproval(approvalId, decision),
  })
  const chooseProjectDirectory = async (): Promise<void> => {
    try {
      const path = await renderer.promptForDirectory()
      if (path === null) return
      pendingProjectPath.current = path
      setConfirmingProjectTrust(true)
    } catch {
      workspaceShell.send({ type: "ERROR", message: "The folder picker could not be opened." })
    }
  }
  const finishProjectTrust = async (acknowledged: boolean): Promise<void> => {
    const path = pendingProjectPath.current
    pendingProjectPath.current = null
    setConfirmingProjectTrust(false)
    if (!acknowledged || path === null) return
    await openProject(path, true)
  }

  return <div style={{ display: "flex", flexDirection: "column", width: "100%", height: "100%", backgroundColor: palette.canvas, color: palette.text }}>
    <div style={{ display: "flex", alignItems: "center", height: metrics.titlebarHeight, paddingLeft: 86, paddingRight: 18, userSelect: "none" }}>
      <text style={{ color: palette.text, fontSize: 13, fontWeight: 650 }}>TAUGENTIC</text><div style={{ flexGrow: 1 }} />
      <text testId="daemon-status" style={{ color: shell.context.phase === "ready" ? palette.accent : shell.context.phase === "unavailable" ? palette.warning : palette.textMuted, fontSize: 11 }}>{shellStatus(shell.context.phase)}</text>
    </div>
    <div style={{ height: 1, backgroundColor: palette.border }} />
    <RuntimeRoutePicker snapshot={shell.context.agentRuntime} draft={shell.context.runtimeDraft} pendingAuthMethodIds={shell.context.pendingAuthMethodIds} onDraft={updateRuntimeDraft} onLogin={(id) => void loginAuthMethod(id, (authorizeUrl) => renderer.openUrl(authorizeUrl))} onLogout={(id) => void logoutAuthProfile(id)} />
    <div testId="workspace-shell" style={{ display: "flex", flexGrow: 1, minHeight: 0, width: "100%", height: "100%" }}>
      <Sidebar snapshot={navigation} state={shell.context.sidebar} selectedProjectId={shell.context.selectedProjectId} conversationTitle={conversationTitle} canCreateConversation={Boolean(selectedProject?.workspaceIds?.[0]) && Boolean(conversationTitle.trim())} onConversationTitleChange={setConversationTitle} onCreateConversation={() => {
        const workspaceId = selectedProject?.workspaceIds?.[0]
        if (!selectedProject || !workspaceId || !conversationTitle.trim()) return
        void createProjectConversation(selectedProject.id, workspaceId, conversationTitle).then((created) => {
          if (created) setConversationTitle("")
        })
      }} dispatch={(action) => {
        workspaceShell.send({ type: "SIDEBAR", action })
        if (action.type === "selectConversation") void selectConversation(action.sessionId)
        if (action.type === "selectProject") selectProject(action.projectId)
        if (action.type === "openProject") void chooseProjectDirectory()
      }} />
      <div style={{ flexGrow: 1, minWidth: 0, minHeight: 0 }}>
        {selectedProject
          ? <Workbench projectId={selectedProject.id} panels={panels} focusPanelId={shell.context.focusPanelId} />
          : <div testId="workspace-awaiting-project" style={{ display: "flex", flexDirection: "column", gap: 10, alignItems: "center", justifyContent: "center", height: "100%" }}><text style={{ color: palette.textMuted }}>Select a project to open the workbench.</text><div testId="open-project" tabIndex={0} onClick={() => void chooseProjectDirectory()} style={{ cursor: "pointer", padding: 9, backgroundColor: palette.panelRaised }}><text>Open project</text></div></div>}
      </div>
    </div>
    {confirmingProjectTrust && <div style={{ position: "absolute", left: 0, right: 0, top: 0, bottom: 0, display: "flex", alignItems: "center", justifyContent: "center" }}><div testId="project-trust-confirmation" style={{ padding: 18, gap: 12, backgroundColor: palette.panelRaised, borderWidth: 1, borderColor: palette.border }}><text>Trust this folder and allow Taugentic to work with its files?</text><div style={{ display: "flex", gap: 8 }}><div testId="decline-project-trust" tabIndex={0} onClick={() => void finishProjectTrust(false)} style={{ cursor: "pointer", padding: 8, backgroundColor: palette.panel }}><text>Cancel</text></div><div testId="confirm-project-trust" tabIndex={0} onClick={() => void finishProjectTrust(true)} style={{ cursor: "pointer", padding: 8, backgroundColor: palette.accent }}><text>Trust and open</text></div></div></div></div>}
    <div style={{ position: "absolute", right: 18, bottom: 18, display: "flex", gap: 8 }}>
      <div testId="close-daemon" tabIndex={0} onClick={() => void closeWorkspaceShell()} style={{ padding: 8, backgroundColor: palette.panelRaised, cursor: "pointer" }}><text>Close connection</text></div>
      <text style={{ color: palette.textFaint, fontSize: 10 }}>{commandRegistry.length} commands</text>
    </div>
  </div>
}

function Workbench({ projectId, panels, focusPanelId }: { projectId: string; panels: ReturnType<typeof panelRegistry>; focusPanelId?: "conversation" | "activity" }) {
  const presentation = useWorkspacePresentation(projectId)
  return <DockWorkspace testId="workspace-dock" layout={presentation.layout} panels={panels} focusPanelId={focusPanelId} onLayoutChange={(layout) => saveWorkspaceLayout(projectId, layout)} onKeyDown={(event) => {
    if (event.key === "1") workspaceShell.send({ type: "FOCUS_PANEL", panelId: "conversation" })
    if (event.key === "2") workspaceShell.send({ type: "FOCUS_PANEL", panelId: "activity" })
  }} style={{ width: "100%", height: "100%" }} accessibilityName="Taugentic workspace" />
}

function useWorkspacePresentation(projectId: string) {
  return useSyncExternalStore(
    (listener) => desktopSettings.subscribe(listener),
    () => workspacePresentation(projectId),
  )
}
