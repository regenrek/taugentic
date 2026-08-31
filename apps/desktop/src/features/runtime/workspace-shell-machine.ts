import { assign, createMachine } from "xstate"

import type {
  AgentRuntimeSelection,
  AgentRuntimeSnapshot,
  DesktopDaemonLifecycleProjection,
  ProjectId,
  RunEventDelta,
  RunStatus,
  SessionId,
  VoiceEvent,
  VoicePermissionState,
  WorkspaceFileAttachmentRequest,
} from "@taugentic/desktop-protocol"

import { compareProtocolU64 } from "../../platform/daemon/protocol-json.js"
import { sidebarReduce, type SidebarAction, type SidebarState } from "../sidebar/sidebar.js"
import type { FocusablePanelId } from "../commands/registry.js"
import type { AssistantMessage } from "../workspace-layout/panels.js"

type ShellPhase = "connecting" | "ready" | "unavailable" | "closed"
export type NavigationPhase = "idle" | "hydrating" | "ready" | "rehydrating" | "error"

export type ShellContext = {
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
  | { type: "RETRY" }
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

export function statusForDeltas(deltas: readonly RunEventDelta[]): RunStatus | undefined {
  for (let index = deltas.length - 1; index >= 0; index -= 1) {
    const event = deltas[index]?.event
    if (event && "run" in event && event.run.kind === "status") return event.run.payload.status
  }
  return undefined
}

export function isTerminalStatus(status: RunStatus | undefined): boolean {
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
    NATIVE_START_REJECTED: { guard: ({ context }) => context.phase === "connecting", actions: assign({ phase: "unavailable", error: () => undefined }) },
    RETRY: { guard: ({ context }) => context.phase === "unavailable", actions: assign({ phase: "connecting", navigation: () => "idle", navigationError: () => undefined, error: () => undefined }) },
    NAVIGATION_READY: { guard: ({ context }) => context.phase === "ready", actions: assign({ navigation: "ready", navigationError: () => undefined }) },
    NAVIGATION_ERROR: { guard: ({ context }) => context.phase === "ready", actions: assign({ navigation: "error", navigationError: "Navigation could not be refreshed. Your connection is still available." }) },
    CLOSED: { actions: assign({ phase: "closed", activeRun: () => undefined, transcriptRunId: () => undefined, error: () => undefined, navigationError: () => undefined }) },
    SET_OBJECTIVE: { guard: ({ context }) => context.phase !== "closed", actions: assign({ objective: ({ event }) => event.objective }) },
    TOGGLE_ATTACHMENT: { guard: ({ context }) => context.phase === "ready" && !context.activeRun, actions: assign({ attachments: ({ context, event }) => context.attachments.some((attachment) => attachment.path === event.attachment.path) ? context.attachments.filter((attachment) => attachment.path !== event.attachment.path) : [...context.attachments, event.attachment] }) },
    REMOVE_ATTACHMENT: { guard: ({ context }) => context.phase === "ready" && !context.activeRun, actions: assign({ attachments: ({ context, event }) => context.attachments.filter((attachment) => attachment.path !== event.path) }) },
    SIDEBAR: { guard: ({ context }) => context.phase !== "closed", actions: assign({ sidebar: ({ context, event }) => sidebarReduce(context.sidebar, event.action) }) },
    SELECTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ sidebar: ({ context, event }) => ({ ...context.sidebar, selectedConversationId: event.sessionId }), attachments: () => [], messages: () => [], runDeltas: () => [], activeRun: () => undefined, transcriptRunId: () => undefined, runStatus: () => undefined, closedSideChatIds: () => [], error: () => undefined, focusPanelId: () => "conversation" }) },
    CONVERSATION_ARCHIVED: { guard: ({ context, event }) => context.phase === "ready" && context.sidebar.selectedConversationId === event.sessionId, actions: assign({ sidebar: ({ context }) => ({ ...context.sidebar, selectedConversationId: undefined }), attachments: () => [], messages: () => [], runDeltas: () => [], activeRun: () => undefined, transcriptRunId: () => undefined, runStatus: () => undefined, closedSideChatIds: () => [], error: () => undefined, focusPanelId: () => undefined }) },
    PROJECT_SELECTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ selectedProjectId: ({ event }) => event.projectId, sidebar: ({ context }) => ({ ...context.sidebar, selectedConversationId: undefined }), attachments: () => [], messages: () => [], runDeltas: () => [], activeRun: () => undefined, transcriptRunId: () => undefined, runStatus: () => undefined, closedSideChatIds: () => [], error: () => undefined }) },
    RUN_STARTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ activeRun: ({ event }) => event.runId, transcriptRunId: ({ event }) => event.runId, runStatus: () => "running", attachments: () => [], messages: () => [], runDeltas: () => [], error: () => undefined, objective: () => "" }) },
    TRANSCRIPT_COMMITTED: { guard: ({ context, event }) => context.transcriptRunId === event.runId && context.activeRun !== event.runId, actions: assign({ transcriptRunId: () => undefined, messages: () => [], runDeltas: () => [] }) },
    RUN_DELTAS: { guard: ({ context, event }) => context.phase !== "closed" && context.transcriptRunId === event.runId, actions: assign(({ context, event }) => { const runDeltas = mergeRunDeltas(context.runDeltas, event.deltas); const runStatus = statusForDeltas(runDeltas) ?? context.runStatus; return { runDeltas, runStatus, activeRun: isTerminalStatus(runStatus) ? undefined : context.activeRun, messages: assistantMessages(runDeltas) } }) },
    AGENT_RUNTIME_READY: { guard: ({ context }) => context.phase !== "closed", actions: assign({ agentRuntime: ({ event }) => event.snapshot }) },
    RUNTIME_DRAFT: { guard: ({ context }) => context.phase === "ready", actions: assign({ pendingSelection: ({ context, event }) => { const runtimeProfileId = event.draft.runtimeProfileId; return runtimeProfileId && runtimeProfileId !== context.pendingSelection?.runtimeProfileId ? { runtimeProfileId } : { ...context.pendingSelection, ...event.draft } } }) },
    AUTH_LOGIN_STARTED: { guard: ({ context }) => context.phase === "ready", actions: assign({ pendingAuthMethodIds: ({ context, event }) => [...context.pendingAuthMethodIds, event.authMethodId], error: () => undefined }) },
    AUTH_LOGIN_FINISHED: { guard: ({ context }) => context.phase !== "closed", actions: assign({ pendingAuthMethodIds: ({ context, event }) => { const index = context.pendingAuthMethodIds.indexOf(event.authMethodId); return index === -1 ? context.pendingAuthMethodIds : context.pendingAuthMethodIds.filter((_, candidateIndex) => candidateIndex !== index) } }) },
    AUTH_LOGIN_FAILED: { guard: ({ context }) => context.phase !== "closed", actions: assign({ error: ({ event }) => event.message }) },
    RUN_CANCELLED: { guard: ({ context, event }) => context.phase !== "closed" && context.activeRun === event.runId, actions: assign({ activeRun: () => undefined, runStatus: () => "cancelled", error: () => undefined }) },
    SIDE_CHAT_OPENED: { guard: ({ context }) => context.phase === "ready", actions: assign({ closedSideChatIds: ({ context, event }) => context.closedSideChatIds.filter((runId) => runId !== event.runId), error: () => undefined }) },
    SIDE_CHAT_CLOSED: { guard: ({ context }) => context.phase !== "closed", actions: assign({ closedSideChatIds: ({ context, event }) => context.closedSideChatIds.includes(event.runId) ? context.closedSideChatIds : [...context.closedSideChatIds, event.runId] }) },
    RUN_STREAM_ERROR: { guard: ({ context, event }) => context.phase !== "closed" && context.activeRun === event.runId, actions: assign({ error: ({ event }) => event.message }) },
    ERROR: { guard: ({ context }) => context.phase !== "closed", actions: assign({ error: ({ event }) => event.message, activeRun: () => undefined }) },
    FOCUS_PANEL: { guard: ({ context }) => context.phase !== "closed", actions: assign({ focusPanelId: ({ event }) => event.panelId }) },
    VOICE_PERMISSION: { guard: ({ context }) => context.phase !== "closed", actions: assign({ voicePermission: ({ event }) => event.permission }) },
    VOICE_STATE: { guard: ({ context }) => context.phase !== "closed", actions: assign({ voice: ({ event }) => event.voice }) },
  },
})
