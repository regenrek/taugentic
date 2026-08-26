import { assign, createActor, createMachine } from "xstate"

import type {
  ApprovalDecision,
  ApprovalId,
  ApprovalRequest,
  ApprovalSnapshotResult,
  DaemonApprovalDecideResult,
  DaemonProjectOpenResult,
  DaemonSessionOpenParams,
  DesktopDaemonLifecycleProjection,
  AgentRuntimeSelection,
  AgentRuntimeSnapshot,
  AuthProfileLoginResult,
  ProjectId,
  RunEventDelta,
  RunEventStreamItem,
  RunStatus,
  SessionId,
  SessionSummary,
  StartRunCommand,
  SubscribeRunEventsResult,
} from "@taugentic/desktop-protocol"

import { invalidateNavigation, navigationQuery, navigationQueryClient, navigationQueryKey } from "../../platform/daemon/navigation-query.js"
import { createDesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import { compareProtocolU64, decodeProtocolJson } from "../../platform/daemon/protocol-json.js"
import { sidebarReduce, type SidebarAction, type SidebarState } from "../sidebar/sidebar.js"
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
  messages: readonly AssistantMessage[]
  approvals: readonly ApprovalRequest[]
  runDeltas: readonly RunEventDelta[]
  activeRun?: string
  runStatus?: RunStatus
  agentRuntime?: AgentRuntimeSnapshot
  runtimeDraft?: Partial<AgentRuntimeSelection>
  pendingAuthMethodIds: readonly string[]
  error?: string
  focusPanelId?: "conversation" | "activity"
}

type ShellEvent =
  | { type: "LIFECYCLE"; projection: DesktopDaemonLifecycleProjection }
  | { type: "NATIVE_START_REJECTED" }
  | { type: "NAVIGATION_READY" }
  | { type: "NAVIGATION_ERROR" }
  | { type: "CLOSED" }
  | { type: "SET_OBJECTIVE"; objective: string }
  | { type: "SIDEBAR"; action: SidebarAction }
  | { type: "SELECTED"; sessionId: SessionId }
  | { type: "PROJECT_SELECTED"; projectId: ProjectId }
  | { type: "RUN_STARTED"; runId: string }
  | { type: "RUN_DELTAS"; deltas: readonly RunEventDelta[] }
  | { type: "APPROVALS_READY"; approvals: readonly ApprovalRequest[] }
  | { type: "APPROVAL_DECIDED"; approvalId: ApprovalId; runStatus: RunStatus }
  | { type: "AGENT_RUNTIME_READY"; snapshot: AgentRuntimeSnapshot }
  | { type: "RUNTIME_DRAFT"; draft: Partial<AgentRuntimeSelection> }
  | { type: "AUTH_LOGIN_STARTED"; authMethodId: string }
  | { type: "AUTH_LOGIN_FINISHED"; authMethodId: string }
  | { type: "AUTH_LOGIN_FAILED"; message: string }
  | { type: "RUN_CANCELLED" }
  | { type: "RUN_STREAM_ERROR"; message: string }
  | { type: "ERROR"; message: string }
  | { type: "FOCUS_PANEL"; panelId: "conversation" | "activity" }

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
    const id = String(stream.itemId ?? stream.turnId ?? stream.runId)
    const current = messages.get(id)
    messages.set(id, { id, text: `${current?.text ?? ""}${stream.frame.delta}` })
  }
  return [...messages.values()]
}

function statusForDeltas(deltas: readonly RunEventDelta[]): RunStatus | undefined {
  for (let index = deltas.length - 1; index >= 0; index -= 1) {
    const event = deltas[index]?.event
    if (event && "run" in event) return event.run.status
  }
  return undefined
}

function isTerminalStatus(status: RunStatus | undefined): boolean {
  return status === "completed" || status === "failed" || status === "budgetExceeded" || status === "cancelled"
}

export const workspaceShellMachine = createMachine({
  types: {} as { context: ShellContext; events: ShellEvent },
  context: { phase: "connecting", navigation: "idle", sidebar: initialSidebar, objective: "", messages: [], approvals: [], runDeltas: [], pendingAuthMethodIds: [] },
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
    CLOSED: { actions: assign({ phase: "closed", activeRun: () => undefined, error: () => undefined, navigationError: () => undefined }) },
    SET_OBJECTIVE: { guard: ({ context }) => context.phase !== "closed", actions: assign({ objective: ({ event }) => event.objective }) },
    SIDEBAR: { guard: ({ context }) => context.phase !== "closed", actions: assign({ sidebar: ({ context, event }) => sidebarReduce(context.sidebar, event.action) }) },
    SELECTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ sidebar: ({ context, event }) => ({ ...context.sidebar, selectedConversationId: event.sessionId }), messages: () => [], approvals: () => [], runDeltas: () => [], activeRun: () => undefined, runStatus: () => undefined, error: () => undefined, focusPanelId: () => "conversation" }) },
    PROJECT_SELECTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ selectedProjectId: ({ event }) => event.projectId, sidebar: ({ context }) => ({ ...context.sidebar, selectedConversationId: undefined }), messages: () => [], approvals: () => [], runDeltas: () => [], activeRun: () => undefined, runStatus: () => undefined, error: () => undefined }) },
    RUN_STARTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ activeRun: ({ event }) => event.runId, runStatus: () => "running", messages: () => [], approvals: () => [], runDeltas: () => [], error: () => undefined, objective: () => "" }) },
    RUN_DELTAS: { guard: ({ context }) => context.phase !== "closed", actions: assign(({ context, event }) => {
      const runDeltas = mergeRunDeltas(context.runDeltas, event.deltas)
      const runStatus = statusForDeltas(runDeltas) ?? context.runStatus
      return {
        runDeltas,
        runStatus,
        activeRun: isTerminalStatus(runStatus) ? undefined : context.activeRun,
        approvals: isTerminalStatus(runStatus) ? [] : context.approvals,
        messages: assistantMessages(runDeltas),
      }
    }) },
    APPROVALS_READY: {
      guard: ({ context }) => context.phase !== "closed",
      actions: assign({ approvals: ({ event }) => event.approvals }),
    },
    APPROVAL_DECIDED: {
      guard: ({ context }) => context.phase !== "closed",
      actions: assign(({ context, event }) => ({
        approvals: isTerminalStatus(event.runStatus)
          ? []
          : context.approvals.filter((approval) => approval.id !== event.approvalId),
        runStatus: event.runStatus,
        activeRun: isTerminalStatus(event.runStatus) ? undefined : context.activeRun,
      })),
    },
    AGENT_RUNTIME_READY: { guard: ({ context }) => context.phase !== "closed", actions: assign({ agentRuntime: ({ event }) => event.snapshot }) },
    RUNTIME_DRAFT: {
      guard: ({ context }) => context.phase === "ready",
      actions: assign({
        runtimeDraft: ({ context, event }) => {
          const selectedRuntimeProfileId = event.draft.runtimeProfileId
          if (selectedRuntimeProfileId && selectedRuntimeProfileId !== context.runtimeDraft?.runtimeProfileId) {
            return { runtimeProfileId: selectedRuntimeProfileId }
          }
          return { ...context.runtimeDraft, ...event.draft }
        },
      }),
    },
    AUTH_LOGIN_STARTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ pendingAuthMethodIds: ({ context, event }) => [...context.pendingAuthMethodIds, event.authMethodId], error: () => undefined }) },
    AUTH_LOGIN_FINISHED: { guard: ({ context }) => context.phase !== "closed", actions: assign({ pendingAuthMethodIds: ({ context, event }) => {
      const index = context.pendingAuthMethodIds.indexOf(event.authMethodId)
      return index === -1 ? context.pendingAuthMethodIds : context.pendingAuthMethodIds.filter((_, candidateIndex) => candidateIndex !== index)
    } }) },
    AUTH_LOGIN_FAILED: { guard: ({ context }) => context.phase !== "closed", actions: assign({ error: ({ event }) => event.message }) },
    RUN_CANCELLED: { guard: ({ context }) => context.phase !== "closed", actions: assign({ activeRun: () => undefined, runStatus: () => "cancelled" }) },
    RUN_STREAM_ERROR: { guard: ({ context }) => context.phase !== "closed", actions: assign({ error: ({ event }) => event.message }) },
    ERROR: { guard: ({ context }) => context.phase !== "closed", actions: assign({ error: ({ event }) => event.message, activeRun: () => undefined }) },
    FOCUS_PANEL: { guard: ({ context }) => context.phase !== "closed", actions: assign({ focusPanelId: ({ event }) => event.panelId }) },
  },
})

export const desktopRuntime = createDesktopRuntime()
export const workspaceShell = createActor(workspaceShellMachine)
let started = false
let closing = false
let navigationRequestId = 0
let approvalRequestId = 0

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
}

function isShellReady(): boolean {
  return !closing && workspaceShell.getSnapshot().context.phase === "ready"
}

async function hydrateNavigation(invalidate: boolean): Promise<void> {
  const requestId = ++navigationRequestId
  try {
    if (invalidate) await invalidateNavigation()
    if (!isShellReady() || requestId !== navigationRequestId) return
    const navigation = await navigationQueryClient.fetchQuery(navigationQuery(desktopRuntime))
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

export function updateRuntimeDraft(draft: Partial<AgentRuntimeSelection>): void {
  if (!isShellReady()) return
  workspaceShell.send({ type: "RUNTIME_DRAFT", draft })
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

export async function selectConversation(sessionId: SessionId): Promise<void> {
  if (!isShellReady()) return
  approvalRequestId += 1
  desktopRuntime.bridge.releaseRunEventSubscription()
  try {
    await desktopRuntime.bridge.attachSession(sessionId)
    if (!isShellReady()) return
    workspaceShell.send({ type: "SELECTED", sessionId })
  } catch {
    workspaceShell.send({ type: "ERROR", message: "This conversation could not be opened. Please choose another session." })
  }
}

export function selectProject(projectId: ProjectId): void {
  if (!isShellReady()) return
  approvalRequestId += 1
  desktopRuntime.bridge.releaseRunEventSubscription()
  workspaceShell.send({ type: "PROJECT_SELECTED", projectId })
}

export async function openProject(path: string, trustAcknowledged: boolean): Promise<void> {
  if (!isShellReady() || !trustAcknowledged) return
  approvalRequestId += 1
  desktopRuntime.bridge.releaseRunEventSubscription()
  try {
    const result = decodeProtocolJson<DaemonProjectOpenResult>(
      await desktopRuntime.bridge.openProject(path, true),
    )
    navigationQueryClient.setQueryData(navigationQueryKey, result.snapshot)
    await invalidateNavigation()
    if (!isShellReady()) return
    workspaceShell.send({ type: "PROJECT_SELECTED", projectId: result.projectId })
  } catch {
    workspaceShell.send({
      type: "ERROR",
      message: "The project could not be opened. Choose another folder and try again.",
    })
  }
}

export async function createProjectConversation(
  projectId: ProjectId,
  workspaceId: string,
  title: string,
): Promise<boolean> {
  const trimmedTitle = title.trim()
  if (!isShellReady() || !trimmedTitle) return false
  try {
    const params: DaemonSessionOpenParams = {
      title: trimmedTitle,
      workspace: { kind: "byProject", projectId, workspaceId },
    }
    const session = decodeProtocolJson<SessionSummary>(
      await desktopRuntime.bridge.openSession(JSON.stringify(params)),
    )
    await invalidateNavigation()
    if (!isShellReady()) return false
    workspaceShell.send({ type: "SELECTED", sessionId: session.id })
    return true
  } catch {
    workspaceShell.send({
      type: "ERROR",
      message: "The conversation could not be created. Choose the project and try again.",
    })
    return false
  }
}

export async function startSelectedRun(): Promise<void> {
  const { phase, sidebar, objective, runtimeDraft, activeRun } = workspaceShell.getSnapshot().context
  const trimmedObjective = objective.trim()
  const sessionId = sidebar.selectedConversationId
  if (phase !== "ready" || !sessionId || !trimmedObjective || !runtimeDraft?.runtimeProfileId || !runtimeDraft.authProfileId || !runtimeDraft.modelId || activeRun) return
  const selection: AgentRuntimeSelection = {
    runtimeProfileId: runtimeDraft.runtimeProfileId,
    authProfileId: runtimeDraft.authProfileId,
    modelId: runtimeDraft.modelId,
  }
  let run: { id: string }
  try {
    run = decodeProtocolJson<{ id: string }>(await desktopRuntime.bridge.startRun(JSON.stringify({ objective: trimmedObjective, selection } satisfies StartRunCommand)))
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The run could not be started. The daemon is still safe to use." })
    return
  }
  workspaceShell.send({ type: "RUN_STARTED", runId: run.id })
  approvalRequestId += 1
  try {
    const replay = decodeProtocolJson<SubscribeRunEventsResult>(await desktopRuntime.bridge.subscribeRunEvents(sessionId, run.id, (eventJson) => {
      try {
        const item = decodeProtocolJson<RunEventStreamItem>(eventJson)
        if (item.payload.kind !== "delta") return
        const delta = item.payload.delta
        workspaceShell.send({ type: "RUN_DELTAS", deltas: [delta] })
        if ("run" in delta.event && delta.event.run.status === "waitingForApproval") {
          void hydrateApprovals(run.id)
        } else if ("approval" in delta.event) {
          void hydrateApprovals(run.id)
        }
      } catch {
        workspaceShell.send({ type: "ERROR", message: "The run event stream ended unexpectedly." })
      }
    }))
    if (replay.events.length) {
      workspaceShell.send({ type: "RUN_DELTAS", deltas: replay.events })
      if (statusForDeltas(replay.events) === "waitingForApproval" || replay.events.some((delta) => "approval" in delta.event)) {
        void hydrateApprovals(run.id)
      }
    }
  } catch {
    workspaceShell.send({ type: "RUN_STREAM_ERROR", message: "The run started, but its event stream could not be opened." })
  }
}

async function hydrateApprovals(runId: string): Promise<void> {
  const requestId = ++approvalRequestId
  try {
    const snapshot = decodeProtocolJson<ApprovalSnapshotResult>(
      await desktopRuntime.bridge.listApprovals(JSON.stringify({ runId })),
    )
    const context = workspaceShell.getSnapshot().context
    if (!isShellReady() || requestId !== approvalRequestId || context.activeRun !== runId) return
    workspaceShell.send({ type: "APPROVALS_READY", approvals: snapshot.items })
  } catch {
    const context = workspaceShell.getSnapshot().context
    if (isShellReady() && requestId === approvalRequestId && context.activeRun === runId) {
      workspaceShell.send({ type: "ERROR", message: "The pending approval could not be loaded." })
    }
  }
}

export async function decideApproval(approvalId: ApprovalId, decision: ApprovalDecision): Promise<void> {
  const { activeRun } = workspaceShell.getSnapshot().context
  if (!isShellReady() || !activeRun) return
  try {
    const result = decodeProtocolJson<DaemonApprovalDecideResult>(
      await desktopRuntime.bridge.decideApproval(JSON.stringify({ approvalId, decision })),
    )
    if (!isShellReady() || workspaceShell.getSnapshot().context.activeRun !== activeRun) return
    workspaceShell.send({ type: "APPROVAL_DECIDED", approvalId, runStatus: result.run.status })
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The approval decision could not be applied." })
  }
}

export async function cancelSelectedRun(): Promise<void> {
  const activeRun = workspaceShell.getSnapshot().context.activeRun
  if (!activeRun) return
  try {
    await desktopRuntime.bridge.cancelRun(activeRun)
    workspaceShell.send({ type: "RUN_CANCELLED" })
  } catch {
    workspaceShell.send({ type: "ERROR", message: "The run could not be cancelled. Check its status before continuing." })
  }
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
