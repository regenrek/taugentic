import { createActor } from "xstate"
import type {
  AgentRuntimeSelection,
  AgentRuntimeSnapshot,
  AuthProfileLoginResult,
  AuthProfilePreferences,
  ContinueRunRequest,
  DesktopDaemonLifecycleProjection,
  DaemonNavigationIntent,
  DaemonProjectOpenResult,
  DaemonSessionOpenParams,
  JoinRunResult,
  NavigationSnapshot,
  ProjectId,
  RunEventDelta,
  RunEventStreamItem,
  RunStatus,
  SessionId,
  SessionSummary,
  SpawnRunRequest,
  StartRunCommand,
  SubscribeRunEventsResult,
  VoicePermissionState,
  WorkspaceFileAttachmentRequest,
  WorkItemKey,
} from "@taugentic/desktop-protocol"
import { invalidateNavigation, navigationQuery, navigationQueryKey, refreshNavigationSnapshot, updateNavigationSnapshot } from "../../platform/daemon/navigation-query.js"
import { conversationBranchesQueryKey, invalidateConversationBranchesForLifecycleRecovery } from "../../platform/daemon/conversation-branches-query.js"
import { createDesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { desktopQueryClient } from "../../platform/daemon/query-client.js"
import { decodeProtocolJson } from "../../platform/daemon/protocol-json.js"
import { runActivityQueryRoot } from "../../platform/daemon/run-activity-query.js"
import { scheduledWorkQueryKey } from "../../platform/daemon/scheduled-work-query.js"
import { invalidateTranscript, transcriptHasCommittedAssistant, transcriptQueryKey } from "../../platform/daemon/transcript-query.js"
import { observeVoice, requestVoicePermission as requestNativeVoicePermission } from "../../platform/daemon/voice-query.js"
import { workItemsQueryKey } from "../../platform/daemon/work-items-query.js"
import { desktopSettings } from "../../platform/settings/desktop-settings.js"
import type { FocusablePanelId } from "../commands/registry.js"
import type { SidebarAction } from "../sidebar/sidebar.js"
import { isTerminalStatus, statusForDeltas, workspaceShellMachine, type ShellContext } from "./workspace-shell-machine.js"
export const desktopRuntime = createDesktopRuntime()
export const workspaceShell = createActor(workspaceShellMachine)
let started = false
let closing = false
let navigationRequestId = 0
let workItemTriggerInFlight = false
let organizationMutationInFlight = false
let selectionRequestId = 0
let startRequestId = 0
let pendingSelection: { sessionId: SessionId; requestId: number } | undefined
const navigationRecovery = createWorkspaceNavigationRecovery(workspaceShell, desktopSettings, (sessionId) => selectConversation(sessionId, false))
/** The sole lifecycle orchestrator; React only observes its XState snapshot. */
export async function startWorkspaceShell(): Promise<void> {
  if (started) return
  started = true
  workspaceShell.start()
  navigationRecovery.applyStoredSidebarPresentation()
  try {
    await desktopRuntime.start()
  } catch {
    workspaceShell.send({ type: "NATIVE_START_REJECTED" })
    return
  }
  observeVoice(
    desktopRuntime,
    (permission) => workspaceShell.send({ type: "VOICE_PERMISSION", permission }),
    (voice) => workspaceShell.send({ type: "VOICE_STATE", voice }),
  )
  try {
    await desktopRuntime.subscribeLifecycle((projection) => {
      void receiveLifecycle(projection)
    })
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The desktop connection could not be started safely." })
  }
}
async function receiveLifecycle(projection: DesktopDaemonLifecycleProjection): Promise<void> {
  const previousNavigation = workspaceShell.getSnapshot().context.navigation
  workspaceShell.send({ type: "LIFECYCLE", projection })
  if (projection.status === "disconnected") {
    navigationRequestId += 1
    return
  }
  if (!isShellReady()) return
  if (projection.invalidated || projection.status === "snapshotRehydrationRequired") {
    await hydrateNavigation(true)
  } else if (previousNavigation === "idle") {
    await hydrateNavigation(false)
  }
  const sessionId = workspaceShell.getSnapshot().context.sidebar.selectedConversationId
  if (sessionId && (projection.invalidated || projection.status === "snapshotRehydrationRequired")) {
    await Promise.all([
      invalidateConversationBranchesForLifecycleRecovery(desktopQueryClient, sessionId),
      desktopQueryClient.invalidateQueries({ queryKey: scheduledWorkQueryKey(sessionId) }),
    ])
  }
}
function isShellReady(): boolean {
  return !closing && workspaceShell.getSnapshot().context.phase === "ready"
}
async function hydrateNavigation(invalidate: boolean): Promise<void> {
  const requestId = ++navigationRequestId
  try {
    if (invalidate) await invalidateNavigation()
    if (!isShellReady() || requestId !== navigationRequestId) return
    const navigation = await desktopQueryClient.fetchQuery(navigationQuery(desktopRuntime))
    if (!isShellReady() || requestId !== navigationRequestId) return
    await navigationRecovery.restoreOnce(navigation)
    if (!isShellReady() || requestId !== navigationRequestId) return
    await hydrateAgentRuntime()
    if (!isShellReady() || requestId !== navigationRequestId) return
    workspaceShell.send({ type: "NAVIGATION_READY" })
  } catch {
    if (isShellReady() && requestId === navigationRequestId) workspaceShell.send({ type: "NAVIGATION_ERROR" })
  }
}
export function createWorkspaceNavigationRecovery(shell: typeof workspaceShell, settings: typeof desktopSettings, attachConversation: (sessionId: SessionId) => Promise<void>) {
  let restored = false
  return {
    applyStoredSidebarPresentation() {
      const navigation = settings.navigation()
      shell.send({ type: "SIDEBAR", action: { type: "selectView", view: navigation.sidebarView } })
      for (const spaceId of navigation.expandedSpaceIds) shell.send({ type: "SIDEBAR", action: { type: "toggleSpace", spaceId } })
    },
    async restoreOnce(navigation: NavigationSnapshot): Promise<void> {
      if (restored) return
      restored = true
      const saved = settings.navigation()
      if (saved.selectedProjectId && navigation.projects?.some((project) => project.id === saved.selectedProjectId)) shell.send({ type: "PROJECT_SELECTED", projectId: saved.selectedProjectId })
      if (saved.selectedSessionId && navigation.conversations?.some((conversation) => conversation.sessionId === saved.selectedSessionId && !conversation.archived)) await attachConversation(saved.selectedSessionId)
    },
    persist() {
      const context = shell.getSnapshot().context
      settings.saveNavigation({ sidebarView: context.sidebar.view, expandedSpaceIds: context.sidebar.expandedSpaceIds, selectedProjectId: context.selectedProjectId, selectedSessionId: context.sidebar.selectedConversationId })
    },
  }
}
export function applySidebarAction(action: SidebarAction): void {
  if (!isShellReady()) return
  workspaceShell.send({ type: "SIDEBAR", action })
  if (action.type === "selectView" || action.type === "toggleSpace") navigationRecovery.persist()
}
async function hydrateAgentRuntime(): Promise<void> {
  const snapshot = decodeProtocolJson<AgentRuntimeSnapshot>(await desktopRuntime.bridge.getAgentRuntime())
  if (isShellReady()) workspaceShell.send({ type: "AGENT_RUNTIME_READY", snapshot })
}
export async function updateRuntimeDraft(draft: Partial<AgentRuntimeSelection>): Promise<void> {
  if (!isShellReady()) return
  workspaceShell.send({ type: "RUNTIME_DRAFT", draft })
  const context = workspaceShell.getSnapshot().context
  const sessionId = context.sidebar.selectedConversationId
  const selection = selectedRuntimeSelection(context)
  if (!sessionId || !selection) return
  try {
    const session = decodeProtocolJson<SessionSummary>(await desktopRuntime.bridge.setSessionNextRunSelection(JSON.stringify({
      selection: { kind: "selected", selection },
    })))
    if (!isShellReady() || workspaceShell.getSnapshot().context.sidebar.selectedConversationId !== sessionId) return
    if (session.nextRunSelection.kind === "selected") {
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: session.nextRunSelection.selection })
    }
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The run route could not be saved for this conversation." })
  }
}
function selectedRuntimeSelection(context: ShellContext): AgentRuntimeSelection | undefined {
  const draft = context.pendingSelection
  if (!draft?.runtimeProfileId || !draft.authProfileId || !draft.modelId) return undefined
  return {
    runtimeProfileId: draft.runtimeProfileId,
    authProfileId: draft.authProfileId,
    modelId: draft.modelId,
  }
}
export function toggleRunAttachment(attachment: WorkspaceFileAttachmentRequest): void {
  if (!isShellReady()) return
  workspaceShell.send({ type: "TOGGLE_ATTACHMENT", attachment })
}
export function removeRunAttachment(path: string): void {
  if (!isShellReady()) return
  workspaceShell.send({ type: "REMOVE_ATTACHMENT", path })
}
export async function loginAuthMethod(
  authMethodId: string,
  openAuthorizeUrl: (authorizeUrl: string) => void,
): Promise<void> {
  if (!isShellReady()) return
  workspaceShell.send({ type: "AUTH_LOGIN_STARTED", authMethodId })
  let pendingAuthProfileId: string | undefined
  let completionAwaitStarted = false
  try {
    const result = decodeProtocolJson<AuthProfileLoginResult>(
      await desktopRuntime.bridge.loginAuthProfile(JSON.stringify({ authMethodId })),
    )
    const challenge = result.challenge
    if (challenge) {
      pendingAuthProfileId = challenge.authProfileId
      if (challenge.method !== "browser" || !challenge.authorizeUrl) {
        throw new Error("The authentication provider returned an unsupported login challenge.")
      }
      const authorizeUrl = new URL(challenge.authorizeUrl)
      if (authorizeUrl.protocol !== "https:") {
        throw new Error("The authentication provider returned an unsafe login URL.")
      }
      openAuthorizeUrl(authorizeUrl.toString())
      completionAwaitStarted = true
      await desktopRuntime.bridge.completeAuthProfileLogin(
        JSON.stringify({ authProfileId: challenge.authProfileId }),
      )
    } else if (result.authProfile.connectionState === "pendingLogin") {
      throw new Error("The authentication provider omitted its login challenge.")
    }
    await hydrateAgentRuntime()
  } catch {
    if (pendingAuthProfileId && !completionAwaitStarted) {
      try {
        await desktopRuntime.bridge.logoutAuthProfile(JSON.stringify({ authProfileId: pendingAuthProfileId }))
      } catch {
        // The completion path already owns terminal provider cleanup.
      }
    }
    workspaceShell.send({ type: "AUTH_LOGIN_FAILED", message: "The authentication profile could not be connected." })
  } finally {
    workspaceShell.send({ type: "AUTH_LOGIN_FINISHED", authMethodId })
  }
}
export async function logoutAuthProfile(authProfileId: string): Promise<void> {
  if (!isShellReady()) return
  try {
    await desktopRuntime.bridge.logoutAuthProfile(JSON.stringify({ authProfileId }))
    await hydrateAgentRuntime()
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The authentication profile could not be disconnected." })
  }
}
export async function replaceAuthProfilePreferences(
  authProfileId: string,
  preferences: AuthProfilePreferences,
): Promise<void> {
  if (!isShellReady()) return
  try {
    const snapshot = decodeProtocolJson<AgentRuntimeSnapshot>(
      await desktopRuntime.bridge.setAuthProfilePreferences(JSON.stringify({ authProfileId, preferences })),
    )
    if (isShellReady()) workspaceShell.send({ type: "AGENT_RUNTIME_READY", snapshot })
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The account preferences could not be saved." })
  }
}
export async function selectConversation(sessionId: SessionId, persist = true, focusPanelId: FocusablePanelId = "conversation"): Promise<void> {
  if (!isShellReady()) return
  const requestId = ++selectionRequestId
  pendingSelection = { sessionId, requestId }
  try {
    const session = decodeProtocolJson<SessionSummary>(await desktopRuntime.bridge.attachSession(sessionId))
    if (!isShellReady() || pendingSelection?.requestId !== requestId || pendingSelection.sessionId !== sessionId) return
    pendingSelection = undefined
    desktopRuntime.bridge.releaseRunEventSubscription()
    workspaceShell.send({ type: "SELECTED", sessionId })
    if (focusPanelId !== "conversation") workspaceShell.send({ type: "FOCUS_PANEL", panelId: focusPanelId })
    if (persist) navigationRecovery.persist()
    if (session.nextRunSelection.kind === "selected") {
      workspaceShell.send({ type: "RUNTIME_DRAFT", draft: session.nextRunSelection.selection })
    }
  } catch {
    if (isShellReady() && requestId === selectionRequestId) {
      pendingSelection = undefined
      workspaceShell.send({ type: "ERROR", message: "This conversation could not be opened. Please choose another session." })
    }
  }
}
export function selectProject(projectId: ProjectId): void {
  if (!isShellReady()) return
  selectionRequestId += 1
  pendingSelection = undefined
  desktopRuntime.bridge.releaseRunEventSubscription()
  workspaceShell.send({ type: "PROJECT_SELECTED", projectId })
  navigationRecovery.persist()
}
export async function openProject(path: string, trustAcknowledged: boolean): Promise<void> {
  if (!isShellReady() || !trustAcknowledged) return
  navigationRequestId += 1
  const navigationEpoch = navigationRequestId
  const requestId = ++selectionRequestId
  pendingSelection = undefined
  desktopRuntime.bridge.releaseRunEventSubscription()
  try {
    const result = decodeProtocolJson<DaemonProjectOpenResult>(
      await desktopRuntime.bridge.openProject(path, true),
    )
    if (!isShellReady() || requestId !== selectionRequestId || navigationEpoch !== navigationRequestId) return
    updateNavigationSnapshot(result.snapshot)
    if (!isShellReady() || requestId !== selectionRequestId || navigationEpoch !== navigationRequestId) return
    await invalidateNavigation()
    if (!isShellReady() || requestId !== selectionRequestId || navigationEpoch !== navigationRequestId) return
    workspaceShell.send({ type: "PROJECT_SELECTED", projectId: result.projectId })
    navigationRecovery.persist()
  } catch {
    if (isShellReady() && requestId === selectionRequestId && navigationEpoch === navigationRequestId) {
      workspaceShell.send({
        type: "ERROR",
        message: "The project could not be opened. Choose another folder and try again.",
      })
    }
  }
}
export async function createProjectConversation(
  projectId: ProjectId,
  workspaceId: string,
  title: string,
): Promise<boolean> {
  return createConversation(title, { kind: "byProject", projectId, workspaceId })
}
async function createConversation(
  title: string,
  workspace: DaemonSessionOpenParams["workspace"],
): Promise<boolean> {
  const trimmedTitle = title.trim()
  if (!isShellReady() || !trimmedTitle) return false
  navigationRequestId += 1
  const navigationEpoch = navigationRequestId
  const requestId = ++selectionRequestId
  pendingSelection = undefined
  try {
    const session = decodeProtocolJson<SessionSummary>(
      await desktopRuntime.bridge.openSession(JSON.stringify({ title: trimmedTitle, workspace } satisfies DaemonSessionOpenParams)),
    )
    if (!isShellReady() || requestId !== selectionRequestId || navigationEpoch !== navigationRequestId) return false
    const navigation = await refreshNavigationSnapshot(desktopRuntime)
    if (!isShellReady() || requestId !== selectionRequestId || navigationEpoch !== navigationRequestId) return false
    if (!navigation.conversations?.some((conversation) => conversation.sessionId === session.id)) {
      workspaceShell.send({
        type: "ERROR",
        message: "The conversation was created, but it is not available in navigation. Refresh navigation and try again.",
      })
      return false
    }
    workspaceShell.send({ type: "SELECTED", sessionId: session.id })
    navigationRecovery.persist()
    return true
  } catch {
    if (isShellReady() && requestId === selectionRequestId && navigationEpoch === navigationRequestId) {
      workspaceShell.send({ type: "ERROR", message: "The conversation could not be created or refreshed. Refresh navigation and try again." })
    }
    return false
  }
}
export async function createStandaloneConversation(workspaceId: string, title: string): Promise<boolean> {
  return createConversation(title, { kind: "byId", id: workspaceId })
}
export async function createTemporaryConversation(workspaceId: string, title: string): Promise<boolean> {
  return createConversation(title, { kind: "byTemporary", workspaceId })
}
export async function closeTemporaryConversation(sessionId: SessionId): Promise<void> {
  await applyOrganizationNavigationIntent(
    { kind: "closeTemporaryConversation", sessionId },
    "The temporary conversation could not be closed.",
    () => {
      const selected = workspaceShell.getSnapshot().context.sidebar.selectedConversationId === sessionId
      if (!selected) return
      selectionRequestId += 1
      pendingSelection = undefined
      desktopRuntime.bridge.releaseRunEventSubscription()
      workspaceShell.send({ type: "CONVERSATION_ARCHIVED", sessionId })
      navigationRecovery.persist()
    },
  )
}
async function applyOrganizationNavigationIntent(
  intent: DaemonNavigationIntent,
  errorMessage: string,
  onApplied?: () => void,
): Promise<boolean> {
  const context = workspaceShell.getSnapshot().context
  if (!isShellReady() || context.activeRun || organizationMutationInFlight) return false
  const lifecycleEpoch = navigationRequestId
  organizationMutationInFlight = true
  try {
    const snapshot = await desktopRuntime.navigationIntent(intent)
    if (!isShellReady() || lifecycleEpoch !== navigationRequestId) return false
    updateNavigationSnapshot(snapshot)
    onApplied?.()
    return true
  } catch {
    if (isShellReady() && lifecycleEpoch === navigationRequestId) {
      workspaceShell.send({ type: "ERROR", message: errorMessage })
    }
    return false
  } finally {
    organizationMutationInFlight = false
  }
}
async function applyConversationNavigationIntent(
  sessionId: SessionId,
  intent: Extract<DaemonNavigationIntent["kind"], "setPinned" | "setArchived">,
  value: boolean,
): Promise<void> {
  const action = intent === "setPinned" ? "pinned" : value ? "archived" : "restored"
  await applyOrganizationNavigationIntent(
    intent === "setPinned"
      ? { kind: "setPinned", sessionId, pinned: value }
      : { kind: "setArchived", sessionId, archived: value },
    `The conversation could not be ${action}.`,
    () => {
      if (intent !== "setArchived" || !value) return
      const selected = workspaceShell.getSnapshot().context.sidebar.selectedConversationId === sessionId
      const pending = pendingSelection?.sessionId === sessionId
      if (selected || pending) {
        selectionRequestId += 1
        pendingSelection = undefined
      }
      if (selected) {
        desktopRuntime.bridge.releaseRunEventSubscription()
        workspaceShell.send({ type: "CONVERSATION_ARCHIVED", sessionId })
        navigationRecovery.persist()
      }
    },
  )
}
export async function createSpace(title: string): Promise<boolean> {
  const trimmedTitle = title.trim()
  if (!trimmedTitle) return false
  return applyOrganizationNavigationIntent(
    { kind: "createSpace", title: trimmedTitle },
    "The space could not be created.",
  )
}
export async function setProjectSpace(projectId: ProjectId, spaceId?: string): Promise<boolean> {
  return applyOrganizationNavigationIntent(
    { kind: "setProjectSpace", projectId, spaceId },
    "The project could not be moved.",
  )
}
export async function setConversationPinned(sessionId: SessionId, pinned: boolean): Promise<void> {
  await applyConversationNavigationIntent(sessionId, "setPinned", pinned)
}
export async function archiveConversation(sessionId: SessionId): Promise<void> {
  await applyConversationNavigationIntent(sessionId, "setArchived", true)
}
export async function restoreConversation(sessionId: SessionId): Promise<void> {
  await applyConversationNavigationIntent(sessionId, "setArchived", false)
}
async function executeStartRun(
  runtime: typeof desktopRuntime,
  shell: typeof workspaceShell,
  recipeId?: string,
): Promise<void> {
  const { phase, sidebar, objective, attachments, activeRun } = shell.getSnapshot().context
  const trimmedObjective = objective.trim()
  const sessionId = sidebar.selectedConversationId
  const selection = selectedRuntimeSelection(shell.getSnapshot().context)
  if (phase !== "ready" || !sessionId || !trimmedObjective || !selection || activeRun) return
  const requestId = ++startRequestId
  const selectionEpoch = selectionRequestId
  let run: { id: string }
  try {
    run = decodeProtocolJson<{ id: string }>(await runtime.bridge.startRun(JSON.stringify({
      objective: trimmedObjective,
      selection,
      attachments: [...attachments],
      recipeId,
    } satisfies StartRunCommand)))
  } catch {
    const current = shell.getSnapshot().context
    if (isShellReady() && requestId === startRequestId && selectionRequestId === selectionEpoch && current.sidebar.selectedConversationId === sessionId) {
      shell.send({ type: "ERROR", message: "The run could not be started. The daemon is still safe to use." })
    }
    return
  }
  const current = shell.getSnapshot().context
  if (!isShellReady() || requestId !== startRequestId || selectionRequestId !== selectionEpoch || current.sidebar.selectedConversationId !== sessionId) return
  await attachStartedRun(runtime, shell, { requestId, selectionEpoch, sessionId, runId: run.id })
}
type StartedRunAttachment = {
  requestId: number
  selectionEpoch: number
  sessionId: SessionId
  runId: string
}
function isCurrentStartedRunAttachment(
  shell: typeof workspaceShell,
  attachment: StartedRunAttachment,
): boolean {
  const context = shell.getSnapshot().context
  return !closing
    && context.phase === "ready"
    && startRequestId === attachment.requestId
    && selectionRequestId === attachment.selectionEpoch
    && context.sidebar.selectedConversationId === attachment.sessionId
}
async function invalidateStartedRunTranscript(
  shell: typeof workspaceShell,
  attachment: StartedRunAttachment,
): Promise<void> {
  if (!isCurrentStartedRunAttachment(shell, attachment)) return
  await invalidateTranscript(attachment.sessionId)
}
async function settleStartedRunTranscript(
  shell: typeof workspaceShell,
  attachment: StartedRunAttachment,
  status: RunStatus,
): Promise<void> {
  if (!isCurrentStartedRunAttachment(shell, attachment)) return
  await invalidateTranscript(attachment.sessionId)
  if (!isCurrentStartedRunAttachment(shell, attachment)) return
  if (status === "completed" && !transcriptHasCommittedAssistant(
    desktopQueryClient.getQueryData(transcriptQueryKey(attachment.sessionId)),
    attachment.runId,
  )) return
  shell.send({ type: "TRANSCRIPT_COMMITTED", runId: attachment.runId })
}
/** The one shell-owned active-run and event-stream attachment lifecycle. */
async function attachStartedRun(
  runtime: typeof desktopRuntime,
  shell: typeof workspaceShell,
  attachment: StartedRunAttachment,
): Promise<void> {
  if (!isCurrentStartedRunAttachment(shell, attachment)) return
  shell.send({ type: "RUN_STARTED", runId: attachment.runId })
  if (!isCurrentStartedRunAttachment(shell, attachment)) return
  await invalidateStartedRunTranscript(shell, attachment)
  if (!isCurrentStartedRunAttachment(shell, attachment)) return
  try {
    const replay = decodeProtocolJson<SubscribeRunEventsResult>(await runtime.bridge.subscribeRunEvents(attachment.sessionId, attachment.runId, (eventJson) => {
      if (!isCurrentStartedRunAttachment(shell, attachment)) return
      try {
        const item = decodeProtocolJson<RunEventStreamItem>(eventJson)
        if (!isCurrentStartedRunAttachment(shell, attachment)) return
        if (item.payload.kind !== "delta") return
        const delta = item.payload.delta
        shell.send({ type: "RUN_DELTAS", runId: attachment.runId, deltas: [delta] })
        if (!isCurrentStartedRunAttachment(shell, attachment)) return
        if ("agentStream" in delta.event && (
          delta.event.agentStream.frame.kind === "assistantTurnCompleted"
          || delta.event.agentStream.frame.kind === "toolCallCompleted"
          || delta.event.agentStream.frame.kind === "pendingStateChanged"
        )) {
          void invalidateStartedRunTranscript(shell, attachment)
        }
        if ("run" in delta.event && delta.event.run.kind === "status" && isTerminalStatus(delta.event.run.payload.status)) {
          void settleStartedRunTranscript(shell, attachment, delta.event.run.payload.status)
        }
      } catch {
        if (isCurrentStartedRunAttachment(shell, attachment)) {
          shell.send({ type: "RUN_STREAM_ERROR", runId: attachment.runId, message: "The run event stream ended unexpectedly." })
        }
      }
    }))
    if (!isCurrentStartedRunAttachment(shell, attachment)) return
    if (replay.events.length) {
      shell.send({ type: "RUN_DELTAS", runId: attachment.runId, deltas: replay.events })
      if (!isCurrentStartedRunAttachment(shell, attachment)) return
      await invalidateStartedRunTranscript(shell, attachment)
      if (!isCurrentStartedRunAttachment(shell, attachment)) return
      const replayStatus = statusForDeltas(replay.events)
      if (replayStatus !== undefined && isTerminalStatus(replayStatus)) {
        await settleStartedRunTranscript(shell, attachment, replayStatus)
      }
    }
  } catch {
    if (isCurrentStartedRunAttachment(shell, attachment)) {
      shell.send({ type: "RUN_STREAM_ERROR", runId: attachment.runId, message: "The run started, but its event stream could not be opened." })
    }
  }
}
export async function startSelectedRun(
  runtime = desktopRuntime,
  shell = workspaceShell,
  recipeId?: string,
): Promise<void> {
  const context = shell.getSnapshot().context
  const profile = context.agentRuntime?.runtimeProfiles?.find(
    (candidate) => candidate.id === context.pendingSelection?.runtimeProfileId,
  )
  if (profile?.executionKind === "realtimeVoice") {
    switch (context.voicePermission) {
      case "authorized":
        break
      case "notDetermined":
        requestVoicePermissionFor(runtime, shell)
        return
      case "denied":
      case "restricted":
        shell.send({
          type: "ERROR",
          message: "Microphone access is required. Grant access in System Settings before starting a voice run.",
        })
        return
    }
  }
  await executeStartRun(runtime, shell, recipeId)
}
/** WorkItem triggering remains a daemon command; the shell only attaches its returned run. */
export async function triggerWorkItem(key: WorkItemKey): Promise<void> {
  if (workItemTriggerInFlight) return
  const context = workspaceShell.getSnapshot().context
  const sessionId = context.sidebar.selectedConversationId
  const selection = selectedRuntimeSelection(context)
  if (!isShellReady() || !sessionId || !selection || context.activeRun) return
  const requestId = ++startRequestId
  const selectionEpoch = selectionRequestId
  workItemTriggerInFlight = true
  let result: Awaited<ReturnType<typeof desktopRuntime.triggerWorkItem>>
  try {
    result = await desktopRuntime.triggerWorkItem(sessionId, { key, selection })
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The work item could not be started. The daemon is still safe to use." })
    return
  } finally {
    workItemTriggerInFlight = false
  }
  const current = workspaceShell.getSnapshot().context
  if (!isShellReady() || requestId !== startRequestId || selectionEpoch !== selectionRequestId || current.sidebar.selectedConversationId !== sessionId || current.activeRun) return
  const attachment = { requestId, selectionEpoch, sessionId, runId: result.run.id }
  await attachStartedRun(desktopRuntime, workspaceShell, attachment)
  if (!isCurrentStartedRunAttachment(workspaceShell, attachment)) return
  await Promise.all([
    invalidateNavigation(),
    desktopQueryClient.invalidateQueries({ queryKey: runActivityQueryRoot }),
    desktopQueryClient.invalidateQueries({ queryKey: transcriptQueryKey(sessionId) }),
    desktopQueryClient.invalidateQueries({ queryKey: conversationBranchesQueryKey(sessionId) }),
    desktopQueryClient.invalidateQueries({ queryKey: workItemsQueryKey }),
  ])
}
export function requestVoicePermission(): void {
  requestVoicePermissionFor(desktopRuntime, workspaceShell)
}
function requestVoicePermissionFor(
  runtime: typeof desktopRuntime,
  shell: typeof workspaceShell,
): void {
  if (shell.getSnapshot().context.phase !== "ready") return
  requestNativeVoicePermission(runtime, (permission) => {
    shell.send({ type: "VOICE_PERMISSION", permission })
  })
}
/** Fresh children are daemon-owned runs; desktop retains only the interaction draft. */
export async function spawnFreshRun(parentRunId: string, objective: string): Promise<void> {
  const context = workspaceShell.getSnapshot().context
  const sessionId = context.sidebar.selectedConversationId
  const selection = selectedRuntimeSelection(context)
  const trimmed = objective.trim()
  if (!isShellReady() || !sessionId || !selection || !trimmed) return
  try {
    await desktopRuntime.spawnRun({
      sessionId,
      parentRunId,
      objective: trimmed,
      selection,
      workspaceScope: "workspaceWrite",
      cleanupPolicy: "deleteOnSuccess",
      plannedWriteFiles: [],
    } satisfies SpawnRunRequest)
    await desktopQueryClient.invalidateQueries({ queryKey: conversationBranchesQueryKey(sessionId) })
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The fresh child could not be started. The daemon is still safe to use." })
  }
}
/** Join reads only daemon/store projections and leaves no desktop result cache. */
export async function joinFreshRun(parentRunId: string, childRunId: string): Promise<JoinRunResult | undefined> {
  const context = workspaceShell.getSnapshot().context
  const sessionId = context.sidebar.selectedConversationId
  if (!isShellReady() || !sessionId) return undefined
  try {
    return await desktopRuntime.joinRun({ sessionId, parentRunId, childRunId })
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The fresh child could not be joined." })
    return undefined
  }
}
async function settleTranscript(sessionId: SessionId, runId: string, status: RunStatus): Promise<void> {
  await invalidateTranscript(sessionId)
  const context = workspaceShell.getSnapshot().context
  if (!isShellReady() || context.sidebar.selectedConversationId !== sessionId) return
  if (status === "completed" && !transcriptHasCommittedAssistant(
    desktopQueryClient.getQueryData(transcriptQueryKey(sessionId)),
    runId,
  )) return
  workspaceShell.send({ type: "TRANSCRIPT_COMMITTED", runId })
}
export async function cancelSelectedRun(): Promise<void> {
  const { activeRun, sidebar } = workspaceShell.getSnapshot().context
  const sessionId = sidebar.selectedConversationId
  if (!activeRun || !sessionId) return
  try {
    await desktopRuntime.bridge.cancelRun(activeRun)
    workspaceShell.send({ type: "RUN_CANCELLED", runId: activeRun })
    await settleTranscript(sessionId, activeRun, "cancelled")
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The run could not be cancelled. Check its status before continuing." })
  }
}
export async function openSideChat(parentRunId: string, parentEventSeq: string): Promise<void> {
  const { phase, sidebar } = workspaceShell.getSnapshot().context
  const sessionId = sidebar.selectedConversationId
  if (phase !== "ready" || !sessionId) return
  try {
    const result = await desktopRuntime.forkRun({ sessionId, parentRunId, parentEventSeq })
    const current = workspaceShell.getSnapshot().context
    if (!isShellReady() || current.sidebar.selectedConversationId !== sessionId) return
    workspaceShell.send({ type: "SIDE_CHAT_OPENED", runId: result.run.id })
    await invalidateTranscript(sessionId)
    await desktopQueryClient.invalidateQueries({ queryKey: conversationBranchesQueryKey(sessionId) })
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The side chat could not be opened at that durable turn." })
  }
}
/** Desktop owns only this call and cache invalidation; branch history and the
 * next user turn remain daemon-owned. */
export async function continueSideChat(runId: string, message: string): Promise<void> {
  const sessionId = workspaceShell.getSnapshot().context.sidebar.selectedConversationId
  const trimmed = message.trim()
  if (!sessionId || !trimmed) return
  try {
    await desktopRuntime.continueRun({ sessionId, runId, message: trimmed } satisfies ContinueRunRequest)
    await Promise.all([
      desktopQueryClient.invalidateQueries({ queryKey: transcriptQueryKey(sessionId) }),
      desktopQueryClient.invalidateQueries({ queryKey: conversationBranchesQueryKey(sessionId) }),
    ])
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The side chat could not continue." })
  }
}
export async function cancelSideChat(runId: string): Promise<void> {
  const { sidebar } = workspaceShell.getSnapshot().context
  const sessionId = sidebar.selectedConversationId
  if (!isShellReady() || !sessionId) return
  try {
    await desktopRuntime.bridge.cancelRun(runId)
    await invalidateTranscript(sessionId)
    await desktopQueryClient.invalidateQueries({ queryKey: conversationBranchesQueryKey(sessionId) })
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The side chat could not be cancelled." })
  }
}
export function closeSideChat(runId: string): void {
  workspaceShell.send({ type: "SIDE_CHAT_CLOSED", runId })
}
/** Panel visibility is presentation-only; run identity and lineage stay in Query. */
export function openSideChatPanel(runId: string): void {
  workspaceShell.send({ type: "SIDE_CHAT_OPENED", runId })
}
export async function closeWorkspaceShell(): Promise<void> {
  if (closing || workspaceShell.getSnapshot().context.phase === "closed") return
  closing = true
  try {
    await desktopRuntime.close()
    navigationRequestId += 1
    workspaceShell.send({ type: "CLOSED" })
  } catch {
    closing = false
    workspaceShell.send({ type: "ERROR", message: "The desktop connection could not be closed safely." })
  }
}
