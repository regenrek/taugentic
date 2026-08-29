import { assign, createActor, createMachine } from "xstate"

import type {
  ContinueRunRequest,
  JoinRunResult,
  DaemonProjectOpenResult,
  DaemonSessionOpenParams,
  DesktopDaemonLifecycleProjection,
  AgentRuntimeSelection,
  AgentRuntimeSnapshot,
  DaemonNavigationIntent,
  AuthProfilePreferences,
  AuthProfileLoginResult,
  ProjectId,
  RunEventDelta,
  RunEventStreamItem,
  RunStatus,
  SessionId,
  SessionSummary,
  StartRunCommand,
  SpawnRunRequest,
  SubscribeRunEventsResult,
  WorkspaceFileAttachmentRequest,
  VoiceEvent,
  VoicePermissionState,
  WorkItemKey,
} from "@taugentic/desktop-protocol"

import { invalidateNavigation, navigationQuery, navigationQueryKey, updateNavigationSnapshot } from "../../platform/daemon/navigation-query.js"
import { desktopQueryClient } from "../../platform/daemon/query-client.js"
import { invalidateTranscript, transcriptHasCommittedAssistant, transcriptQueryKey } from "../../platform/daemon/transcript-query.js"
import { conversationBranchesQueryKey, invalidateConversationBranchesForLifecycleRecovery } from "../../platform/daemon/conversation-branches-query.js"
import { runActivityQueryRoot } from "../../platform/daemon/run-activity-query.js"
import { workItemsQueryKey } from "../../platform/daemon/work-items-query.js"
import { createDesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { observeVoice, requestVoicePermission as requestNativeVoicePermission } from "../../platform/daemon/voice-query.js"
import { compareProtocolU64, decodeProtocolJson } from "../../platform/daemon/protocol-json.js"
import { sidebarReduce, type SidebarAction, type SidebarState } from "../sidebar/sidebar.js"
import type { FocusablePanelId } from "../commands/registry.js"
import type { AssistantMessage } from "../workspace-layout/panels.js"

type ShellPhase = "connecting" | "ready" | "unavailable" | "closed"
export type NavigationPhase = "idle" | "hydrating" | "ready" | "rehydrating" | "error"

type ShellContext = {
  phase: ShellPhase
  navigation: NavigationPhase
  navigationError?: string
  sidebar: SidebarState
  selectedProjectId?: ProjectId
  objective: string
  attachments: readonly WorkspaceFileAttachmentRequest[]
  messages: readonly AssistantMessage[]
  runDeltas: readonly RunEventDelta[]
  activeRun?: string
  transcriptRunId?: string
  runStatus?: RunStatus
  agentRuntime?: AgentRuntimeSnapshot
  pendingSelection?: Partial<AgentRuntimeSelection>
  pendingAuthMethodIds: readonly string[]
  error?: string
  focusPanelId?: FocusablePanelId
  closedSideChatIds: readonly string[]
  voicePermission: VoicePermissionState
  voice?: VoiceEvent
}

type ShellEvent =
  | { type: "LIFECYCLE"; projection: DesktopDaemonLifecycleProjection }
  | { type: "NATIVE_START_REJECTED" }
  | { type: "NAVIGATION_READY" }
  | { type: "NAVIGATION_ERROR" }
  | { type: "CLOSED" }
  | { type: "SET_OBJECTIVE"; objective: string }
  | { type: "TOGGLE_ATTACHMENT"; attachment: WorkspaceFileAttachmentRequest }
  | { type: "REMOVE_ATTACHMENT"; path: string }
  | { type: "SIDEBAR"; action: SidebarAction }
  | { type: "SELECTED"; sessionId: SessionId }
  | { type: "CONVERSATION_ARCHIVED"; sessionId: SessionId }
  | { type: "PROJECT_SELECTED"; projectId: ProjectId }
  | { type: "RUN_STARTED"; runId: string }
  | { type: "TRANSCRIPT_COMMITTED"; runId: string }
  | { type: "RUN_DELTAS"; runId: string; deltas: readonly RunEventDelta[] }
  | { type: "AGENT_RUNTIME_READY"; snapshot: AgentRuntimeSnapshot }
  | { type: "RUNTIME_DRAFT"; draft: Partial<AgentRuntimeSelection> }
  | { type: "AUTH_LOGIN_STARTED"; authMethodId: string }
  | { type: "AUTH_LOGIN_FINISHED"; authMethodId: string }
  | { type: "AUTH_LOGIN_FAILED"; message: string }
  | { type: "RUN_CANCELLED"; runId: string }
  | { type: "SIDE_CHAT_OPENED"; runId: string }
  | { type: "SIDE_CHAT_CLOSED"; runId: string }
  | { type: "RUN_STREAM_ERROR"; runId: string; message: string }
  | { type: "ERROR"; message: string }
  | { type: "FOCUS_PANEL"; panelId: FocusablePanelId }
  | { type: "VOICE_PERMISSION"; permission: VoicePermissionState }
  | { type: "VOICE_STATE"; voice: VoiceEvent }

const initialSidebar: SidebarState = { view: "spaces", filter: "", expandedSpaceIds: [] }

function mergeRunDeltas(current: readonly RunEventDelta[], incoming: readonly RunEventDelta[]): RunEventDelta[] {
  const bySequence = new Map(current.map((delta) => [delta.seq, delta]))
  for (const delta of incoming) bySequence.set(delta.seq, delta)
  return [...bySequence.values()].sort((left, right) => compareProtocolU64(left.seq, right.seq))
}

function assistantMessages(deltas: readonly RunEventDelta[]): AssistantMessage[] {
  const messages = new Map<string, AssistantMessage>()
  for (const delta of deltas) {
    const event = delta.event
    if (!("agentStream" in event)) continue
    const stream = event.agentStream
    if (stream.frame.kind !== "assistantMessageDelta") continue
    const id = String(stream.turnId ?? stream.itemId ?? stream.runId)
    const current = messages.get(id)
    messages.set(id, { id, text: `${current?.text ?? ""}${stream.frame.delta}` })
  }
  return [...messages.values()]
}

function statusForDeltas(deltas: readonly RunEventDelta[]): RunStatus | undefined {
  for (let index = deltas.length - 1; index >= 0; index -= 1) {
    const event = deltas[index]?.event
    if (event && "run" in event && event.run.kind === "status") return event.run.payload.status
  }
  return undefined
}

function isTerminalStatus(status: RunStatus | undefined): boolean {
  return status === "completed" || status === "failed" || status === "budgetExceeded" || status === "cancelled"
}

export const workspaceShellMachine = createMachine({
  types: {} as { context: ShellContext; events: ShellEvent },
  context: { phase: "connecting", navigation: "idle", sidebar: initialSidebar, objective: "", attachments: [], messages: [], runDeltas: [], pendingAuthMethodIds: [], closedSideChatIds: [], voicePermission: "notDetermined" },
  on: {
    LIFECYCLE: {
      guard: ({ context }) => context.phase !== "closed",
      actions: assign({
        phase: ({ event }) => event.projection.status === "disconnected" ? "unavailable" : "ready",
        navigation: ({ context, event }) => {
          if (event.projection.status === "disconnected") return context.navigation
          if (event.projection.invalidated || event.projection.status === "snapshotRehydrationRequired") return "rehydrating"
          return context.navigation === "idle" ? "hydrating" : context.navigation
        },
        navigationError: ({ context, event }) => (
          event.projection.status === "disconnected"
          || event.projection.invalidated
          || event.projection.status === "snapshotRehydrationRequired"
            ? undefined
            : context.navigationError
        ),
      }),
    },
    NATIVE_START_REJECTED: {
      guard: ({ context }) => context.phase === "connecting",
      actions: assign({ phase: "unavailable", error: () => undefined }),
    },
    NAVIGATION_READY: {
      guard: ({ context }) => context.phase === "ready",
      actions: assign({ navigation: "ready", navigationError: () => undefined }),
    },
    NAVIGATION_ERROR: {
      guard: ({ context }) => context.phase === "ready",
      actions: assign({
        navigation: "error",
        navigationError: "Navigation could not be refreshed. Your connection is still available.",
      }),
    },
    CLOSED: { actions: assign({ phase: "closed", activeRun: () => undefined, transcriptRunId: () => undefined, error: () => undefined, navigationError: () => undefined }) },
    SET_OBJECTIVE: { guard: ({ context }) => context.phase !== "closed", actions: assign({ objective: ({ event }) => event.objective }) },
    TOGGLE_ATTACHMENT: {
      guard: ({ context }) => context.phase === "ready" && !context.activeRun,
      actions: assign({ attachments: ({ context, event }) => (
        context.attachments.some((attachment) => attachment.path === event.attachment.path)
          ? context.attachments.filter((attachment) => attachment.path !== event.attachment.path)
          : [...context.attachments, event.attachment]
      ) }),
    },
    REMOVE_ATTACHMENT: {
      guard: ({ context }) => context.phase === "ready" && !context.activeRun,
      actions: assign({ attachments: ({ context, event }) => context.attachments.filter((attachment) => attachment.path !== event.path) }),
    },
    SIDEBAR: { guard: ({ context }) => context.phase !== "closed", actions: assign({ sidebar: ({ context, event }) => sidebarReduce(context.sidebar, event.action) }) },
    SELECTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ sidebar: ({ context, event }) => ({ ...context.sidebar, selectedConversationId: event.sessionId }), attachments: () => [], messages: () => [], runDeltas: () => [], activeRun: () => undefined, transcriptRunId: () => undefined, runStatus: () => undefined, closedSideChatIds: () => [], error: () => undefined, focusPanelId: () => "conversation" }) },
    CONVERSATION_ARCHIVED: {
      guard: ({ context, event }) => context.phase === "ready" && context.sidebar.selectedConversationId === event.sessionId,
      actions: assign({
        sidebar: ({ context }) => ({ ...context.sidebar, selectedConversationId: undefined }),
        attachments: () => [], messages: () => [], runDeltas: () => [], activeRun: () => undefined,
        transcriptRunId: () => undefined, runStatus: () => undefined, closedSideChatIds: () => [],
        error: () => undefined, focusPanelId: () => undefined,
      }),
    },
    PROJECT_SELECTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ selectedProjectId: ({ event }) => event.projectId, sidebar: ({ context }) => ({ ...context.sidebar, selectedConversationId: undefined }), attachments: () => [], messages: () => [], runDeltas: () => [], activeRun: () => undefined, transcriptRunId: () => undefined, runStatus: () => undefined, closedSideChatIds: () => [], error: () => undefined }) },
    RUN_STARTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ activeRun: ({ event }) => event.runId, transcriptRunId: ({ event }) => event.runId, runStatus: () => "running", attachments: () => [], messages: () => [], runDeltas: () => [], error: () => undefined, objective: () => "" }) },
    TRANSCRIPT_COMMITTED: {
      guard: ({ context, event }) => context.transcriptRunId === event.runId && context.activeRun !== event.runId,
      actions: assign({ transcriptRunId: () => undefined, messages: () => [], runDeltas: () => [] }),
    },
    RUN_DELTAS: { guard: ({ context, event }) => context.phase !== "closed" && context.transcriptRunId === event.runId, actions: assign(({ context, event }) => {
      const runDeltas = mergeRunDeltas(context.runDeltas, event.deltas)
      const runStatus = statusForDeltas(runDeltas) ?? context.runStatus
      return {
        runDeltas,
        runStatus,
        activeRun: isTerminalStatus(runStatus) ? undefined : context.activeRun,
        messages: assistantMessages(runDeltas),
      }
    }) },
    AGENT_RUNTIME_READY: { guard: ({ context }) => context.phase !== "closed", actions: assign({ agentRuntime: ({ event }) => event.snapshot }) },
    RUNTIME_DRAFT: {
      guard: ({ context }) => context.phase === "ready",
      actions: assign({
        pendingSelection: ({ context, event }) => {
          const selectedRuntimeProfileId = event.draft.runtimeProfileId
          if (selectedRuntimeProfileId && selectedRuntimeProfileId !== context.pendingSelection?.runtimeProfileId) {
            return { runtimeProfileId: selectedRuntimeProfileId }
          }
          return { ...context.pendingSelection, ...event.draft }
        },
      }),
    },
    AUTH_LOGIN_STARTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ pendingAuthMethodIds: ({ context, event }) => [...context.pendingAuthMethodIds, event.authMethodId], error: () => undefined }) },
    AUTH_LOGIN_FINISHED: { guard: ({ context }) => context.phase !== "closed", actions: assign({ pendingAuthMethodIds: ({ context, event }) => {
      const index = context.pendingAuthMethodIds.indexOf(event.authMethodId)
      return index === -1 ? context.pendingAuthMethodIds : context.pendingAuthMethodIds.filter((_, candidateIndex) => candidateIndex !== index)
    } }) },
    AUTH_LOGIN_FAILED: { guard: ({ context }) => context.phase !== "closed", actions: assign({ error: ({ event }) => event.message }) },
    RUN_CANCELLED: {
      guard: ({ context, event }) => context.phase !== "closed" && context.activeRun === event.runId,
      actions: assign({
        activeRun: () => undefined,
        runStatus: () => "cancelled",
        error: () => undefined,
      }),
    },
    SIDE_CHAT_OPENED: {
      guard: ({ context }) => context.phase === "ready",
      actions: assign({ closedSideChatIds: ({ context, event }) => context.closedSideChatIds.filter((runId) => runId !== event.runId), error: () => undefined }),
    },
    SIDE_CHAT_CLOSED: {
      guard: ({ context }) => context.phase !== "closed",
      actions: assign({ closedSideChatIds: ({ context, event }) => context.closedSideChatIds.includes(event.runId) ? context.closedSideChatIds : [...context.closedSideChatIds, event.runId] }),
    },
    RUN_STREAM_ERROR: {
      guard: ({ context, event }) => context.phase !== "closed" && context.activeRun === event.runId,
      actions: assign({ error: ({ event }) => event.message }),
    },
    ERROR: { guard: ({ context }) => context.phase !== "closed", actions: assign({ error: ({ event }) => event.message, activeRun: () => undefined }) },
    FOCUS_PANEL: { guard: ({ context }) => context.phase !== "closed", actions: assign({ focusPanelId: ({ event }) => event.panelId }) },
    VOICE_PERMISSION: { guard: ({ context }) => context.phase !== "closed", actions: assign({ voicePermission: ({ event }) => event.permission }) },
    VOICE_STATE: { guard: ({ context }) => context.phase !== "closed", actions: assign({ voice: ({ event }) => event.voice }) },
  },
})

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

/** The sole lifecycle orchestrator; React only observes its XState snapshot. */
export async function startWorkspaceShell(): Promise<void> {
  if (started) return
  started = true
  workspaceShell.start()
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
    await invalidateConversationBranchesForLifecycleRecovery(desktopQueryClient, sessionId)
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
    await hydrateAgentRuntime()
    if (!isShellReady() || requestId !== navigationRequestId) return
    workspaceShell.send({ type: "NAVIGATION_READY" })
  } catch {
    if (isShellReady() && requestId === navigationRequestId) workspaceShell.send({ type: "NAVIGATION_ERROR" })
  }
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

export async function selectConversation(sessionId: SessionId): Promise<void> {
  if (!isShellReady()) return
  const requestId = ++selectionRequestId
  pendingSelection = { sessionId, requestId }
  try {
    const session = decodeProtocolJson<SessionSummary>(await desktopRuntime.bridge.attachSession(sessionId))
    if (!isShellReady() || pendingSelection?.requestId !== requestId || pendingSelection.sessionId !== sessionId) return
    pendingSelection = undefined
    desktopRuntime.bridge.releaseRunEventSubscription()
    workspaceShell.send({ type: "SELECTED", sessionId })
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
    await invalidateNavigation()
    if (!isShellReady() || requestId !== selectionRequestId || navigationEpoch !== navigationRequestId) return false
    workspaceShell.send({ type: "SELECTED", sessionId: session.id })
    return true
  } catch {
    if (isShellReady() && requestId === selectionRequestId && navigationEpoch === navigationRequestId) {
      workspaceShell.send({ type: "ERROR", message: "The conversation could not be created. The daemon is still safe to use." })
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
