import { Combobox, ComboboxContent, VirtualList, useGpuixRequired, type DockPanel } from "@regenrek/gpuix-react"
import type { AgentTurnRow, ArtifactSummary, BoundedFileContent, CapsuleRecipe, JoinRunResult, NativeImagePreview, RunLineageGraphResult, RunListEntry, RunStatus, RuntimeLanePendingState, SessionId, WorkspaceFileAttachmentRequest } from "@taugentic/desktop-protocol"
import { Fragment, memo, useCallback, useEffect, useMemo, useRef, useState } from "react"

import { palette } from "../../app/theme.js"
import { ArtifactsPanel, artifactDisplayName, type ArtifactPanelState } from "../artifacts/artifacts-panel.js"
import { PullRequestsPanel } from "../code-host/pull-requests-panel.js"
import { RunActivityPanel } from "../run-activity/run-activity-panel.js"
import type { ReturnTypeUseRunActivity } from "../run-activity/types.js"
import type { WorkbenchCodeHostState } from "../code-host/use-workbench-code-host.js"
import { DiffPanel, FileTreePanel, ImagePanel, PdfPanel, SourcePanel, type FilePanelState } from "../files/file-panels.js"
import { GitPanel } from "../git/git-panel.js"
import type { WorkbenchGitState } from "../git/use-workbench-git.js"
import { TerminalPanel } from "../terminal/terminal-panel.js"
import type { WorkbenchTerminalState } from "../terminal/use-workbench-terminal.js"
import { ThreadWorkspacePanel } from "../thread-workspace/thread-workspace-panel.js"
import type { ThreadWorkspacePanelState } from "../thread-workspace/use-thread-workspace.js"
import { commandById, commandRegistry, type CommandDispatcher } from "../commands/registry.js"
import { ConversationBranchGraph } from "../conversation-branches/branch-graph.js"
import { VoicePanel, type VoicePanelProps } from "../voice/voice-panel.js"

export type AssistantMessage = { id: string; text: string }

function activates(event: { key?: string }): boolean { return event.key === "enter" || event.key === "space" }

export type ConversationPanelProps = {
  title: string
  selectedConversationId?: SessionId
  transcriptRows: readonly AgentTurnRow[]
  transcriptLoading: boolean
  transcriptError?: string
  hasOlderTranscript: boolean
  loadingOlderTranscript: boolean
  onLoadOlderTranscript(): void
  messages: readonly AssistantMessage[]
  approvals?: readonly import("@taugentic/desktop-protocol").ApprovalRequest[]
  objective: string
  attachments: readonly WorkspaceFileAttachmentRequest[]
  error?: string
  runStatus?: RunStatus
  onObjectiveChange(value: string): void
  onRemoveAttachment(path: string): void
  recipes?: readonly CapsuleRecipe[]
  recipesLoading?: boolean
  recipesError?: string
  selectedRecipeId?: string
  onSelectRecipe?(recipeId?: string): void
  /** All command-shaped controls consume this owner. */
  commands: CommandDispatcher
  onDecideApproval?(approvalId: import("@taugentic/desktop-protocol").ApprovalId, decision: import("@taugentic/desktop-protocol").ApprovalDecision): void
  branches?: readonly RunListEntry[]
  branchGraph?: RunLineageGraphResult
  branchGraphState?: "loading" | "offline" | "error" | "ready"
  cortexVisible?: boolean
  sideChats?: readonly RunListEntry[]
  onOpenSideChat?(parentRunId: string, parentEventSeq: string): void
  onCancelSideChat?(runId: string): void
  onCloseSideChat?(runId: string): void
  onContinueSideChat?(runId: string, message: string): void
  onOpenSideChatPanel?(runId: string): void
  onPinThreadWorkspace?(runId: string, cursor: string): void
  onSpawnFresh?(parentRunId: string, objective: string): Promise<void>
  onJoinFresh?(parentRunId: string, childRunId: string): Promise<JoinRunResult | undefined>
  voice?: VoicePanelProps
}

export type WorkbenchPanelProps = ConversationPanelProps & {
  files: FilePanelState
  terminal: WorkbenchTerminalState
  git: WorkbenchGitState
  codeHost: WorkbenchCodeHostState
  threadWorkspace: ThreadWorkspacePanelState
  runActivity?: ReturnTypeUseRunActivity | undefined
  openUrl(url: string): void
  artifacts: ArtifactPanelState & {
    selectedContent?: BoundedFileContent
    selectedImagePreview?: NativeImagePreview
    contentLoading: boolean
    contentError?: string
    pdfPageIndex: number
    setPdfPageIndex(page: number): void
  }
}

function selectedPreview(props: WorkbenchPanelProps): {
  label?: string
  content?: BoundedFileContent
  image?: NativeImagePreview
  loading: boolean
  error?: string
  pdfPageIndex: number
  setPdfPageIndex(page: number): void
  artifact?: ArtifactSummary
} {
  if (props.artifacts.selectedArtifact) {
    return {
      label: artifactDisplayName(props.artifacts.selectedArtifact),
      content: props.artifacts.selectedContent,
      image: props.artifacts.selectedImagePreview,
      loading: props.artifacts.contentLoading,
      error: props.artifacts.contentError,
      pdfPageIndex: props.artifacts.pdfPageIndex,
      setPdfPageIndex: props.artifacts.setPdfPageIndex,
      artifact: props.artifacts.selectedArtifact,
    }
  }
  return {
    label: props.files.selectedPath,
    content: props.files.selectedContent,
    image: props.files.selectedImagePreview,
    loading: props.files.contentLoading,
    error: props.files.contentError,
    pdfPageIndex: props.files.pdfPageIndex,
    setPdfPageIndex: props.files.setPdfPageIndex,
  }
}

export function panelRegistry(props: WorkbenchPanelProps): readonly DockPanel[] {
  const preview = selectedPreview(props)
  const patch = preview.artifact?.kind === "Patch" || preview.label?.endsWith(".diff") || preview.label?.endsWith(".patch")
    ? preview
    : { ...preview, content: undefined }
  return [
    {
      id: "files",
      label: "Files",
      content: <FileTreePanel {...props.files} />,
      closable: false,
    },
    {
      id: "artifacts",
      label: "Artifacts",
      content: <ArtifactsPanel {...props.artifacts} />,
      closable: false,
    },
    {
      id: "conversation",
      label: "Conversation",
      content: <ConversationPanel {...props} />,
      closable: false,
    },
    {
      id: "activity",
      label: "Activity",
      content: props.runActivity ? <RunActivityPanel activity={props.runActivity} /> : <div testId="run-activity-panel" style={{ padding: 20 }}><text style={{ color: palette.textMuted }}>Run activity is unavailable.</text></div>,
      closable: false,
    },
    {
      id: "thread-workspace",
      label: "Thread workspace",
      content: <ThreadWorkspacePanel workspace={props.threadWorkspace} />,
      closable: false,
    },
    {
      id: "git",
      label: "Git",
      content: <GitPanel git={props.git} codeHost={props.codeHost} />,
      closable: false,
    },
    {
      id: "pull-requests",
      label: "Pull requests",
      content: <PullRequestsPanel codeHost={props.codeHost} openUrl={props.openUrl} />,
      closable: false,
    },
    {
      id: "terminal",
      label: "Terminal",
      content: <TerminalPanel terminal={props.terminal} />,
      closable: false,
    },
    {
      id: "source",
      label: "Source",
      content: <SourcePanel {...props.files} />,
      closable: false,
    },
    {
      id: "diff",
      label: "Diff",
      content: <DiffPanel {...patch} />,
      closable: false,
    },
    {
      id: "image",
      label: "Image",
      content: <ImagePanel {...preview} />,
      closable: false,
    },
    {
      id: "pdf",
      label: "PDF",
      content: <PdfPanel {...preview} onOpenExternal={preview.artifact ? undefined : props.files.openExternal} />,
      closable: false,
    },
  ]
}

export function ConversationPanel(props: ConversationPanelProps) {
  const renderer = useGpuixRequired()
  const composer = useRef<any>(null)
  const [freshSpawnParentRunId, setFreshSpawnParentRunId] = useState<string>()
  const slashCommands = commandRegistry.filter((command) => command.title.toLowerCase().includes(props.objective.slice(1).toLowerCase()))
  const slashOpen = props.objective.startsWith("/")
  const closeSlash = () => { props.onObjectiveChange(""); if (composer.current) renderer.focusElement?.(composer.current.id) }
  return <div testId="conversation-panel" style={{ display: "flex", flexDirection: "column", height: "100%", padding: 24, gap: 14, minWidth: 0 }}>
    <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
      <text style={{ color: palette.text, fontSize: 21, fontWeight: 650 }}>{props.title}</text>
      {props.runStatus && <text testId="run-status" style={{ color: props.runStatus === "failed" || props.runStatus === "budgetExceeded" ? "#f08080" : props.runStatus === "running" ? palette.accent : palette.textMuted, fontSize: 11 }}>{runStatusLabel(props.runStatus)}</text>}
    </div>
    {props.error && <text testId="daemon-error" style={{ color: "#f08080", fontSize: 12 }}>{props.error}</text>}
    <VoicePanel {...(props.voice ?? { visible: false, permission: "restricted", onRequestPermission: () => {} })} />
    <div style={{ display: "flex", flexGrow: 1, minHeight: 0, gap: 12 }}>
      <Transcript
        sessionId={props.selectedConversationId}
        rows={props.transcriptRows}
        liveMessages={props.messages}
        loading={props.transcriptLoading}
        error={props.transcriptError}
        hasOlder={props.hasOlderTranscript}
        loadingOlder={props.loadingOlderTranscript}
        onLoadOlder={props.onLoadOlderTranscript}
        hiddenRunIds={new Set((props.sideChats ?? []).map((chat) => chat.id))}
        onOpenSideChat={props.onOpenSideChat}
        onOpenFreshSpawn={(parentRunId) => setFreshSpawnParentRunId(parentRunId)}
        onPinThreadWorkspace={props.onPinThreadWorkspace}
      />
      <div style={{ display: "flex", flexDirection: "column", flexBasis: 320, minWidth: 260, gap: 10, overflow: "scroll" }}>
        <ConversationBranchGraph graph={props.branchGraph} state={props.branchGraphState} visible={props.cortexVisible === true} onOpen={(runId) => props.onOpenSideChatPanel?.(runId)} />
        {freshSpawnParentRunId && <FreshSpawnComposer parentRunId={freshSpawnParentRunId} onDismiss={() => setFreshSpawnParentRunId(undefined)} onSpawn={props.onSpawnFresh} />}
        {(props.branches ?? []).filter((branch) => branch.relationship.kind === "freshSpawn").map((branch) => <FreshSpawnPanel key={branch.id} run={branch} onJoin={props.onJoinFresh} />)}
        {!!props.sideChats?.length && <div testId="side-chat-stack" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {props.sideChats.map((chat) => <SideChatPanel key={chat.id} run={chat} rows={props.transcriptRows.filter((row) => row.runId === chat.id)} onCancel={props.onCancelSideChat} onClose={props.onCloseSideChat} onContinue={props.onContinueSideChat} />)}
        </div>}
      </div>
    </div>
    {!!props.attachments.length && <div testId="run-attachments" style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>{props.attachments.map((attachment) => <Fragment key={`${attachment.path}:${attachment.expectedRevision}`}><div style={{ display: "flex", alignItems: "center", gap: 6, padding: 6, borderWidth: 1, borderColor: palette.border, borderRadius: 6 }}><text style={{ color: palette.textMuted, fontSize: 10, userSelect: "text" }}>{attachment.path}</text><div testId={`remove-attachment-${attachment.path}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Remove attachment ${attachment.path}`} accessibilityDisabled={false} onClick={() => props.onRemoveAttachment(attachment.path)} onKeyDown={(event) => { if (activates(event)) props.onRemoveAttachment(attachment.path) }} style={{ cursor: "pointer" }}><text style={{ color: palette.textFaint, fontSize: 10 }}>×</text></div></div></Fragment>)}</div>}
    <RecipeComposer recipes={props.recipes} loading={props.recipesLoading} error={props.recipesError} selectedRecipeId={props.selectedRecipeId} onSelectRecipe={props.onSelectRecipe} />
    <Combobox open={slashOpen} onOpenChange={(open) => { if (!open) closeSlash() }} items={[]}><textarea ref={composer} testId="run-objective" autoFocus value={props.objective} placeholder="Describe the work to run" minRows={2} maxRows={8} onKeyDown={(event) => { if (event.key === "escape" && slashOpen) closeSlash() }} onChange={(event) => props.onObjectiveChange(event.value ?? "")} style={{ minHeight: 54, padding: 10, borderWidth: 1, borderColor: palette.border, borderRadius: 8, color: palette.text, backgroundColor: palette.panel }} />
      {slashOpen && <ComboboxContent testId="composer-slash-completion" side="top" onMouseDownOutside={closeSlash} accessibilityRole="menu" accessibilityName="Composer slash commands" style={{ padding: 4, backgroundColor: palette.panelRaised }}>{slashCommands.map((command) => { const enabled = props.commands.enabled(command.id); const dispatch = () => { if (enabled && props.commands.dispatch(command.id)) closeSlash() }; return <Fragment key={command.id}><div testId={`composer-command-${command.id}`} tabIndex={enabled ? 0 : -1} accessibilityRole="menuitem" accessibilityName={command.title} accessibilityDisabled={!enabled} onClick={dispatch} onKeyDown={(event) => { if (event.key === "escape") closeSlash(); if (activates(event)) dispatch() }} style={{ padding: 6, cursor: enabled ? "pointer" : "default" }}><text>{command.title}</text></div></Fragment> })}</ComboboxContent>}
    </Combobox>
    <div style={{ display: "flex", gap: 8 }}>
      {(["start-run", "cancel-run"] as const).map((id) => { const enabled = props.commands.enabled(id); const title = commandById(id)!.title; const dispatch = () => { if (enabled) props.commands.dispatch(id) }; return <Fragment key={id}><div testId={id} tabIndex={enabled ? 0 : -1} accessibilityRole="button" accessibilityName={title} accessibilityDisabled={!enabled} onClick={dispatch} onKeyDown={(event) => { if (activates(event)) dispatch() }} style={{ padding: 8, backgroundColor: enabled ? palette.accentDim : palette.panelRaised, color: enabled ? palette.text : palette.textFaint, cursor: enabled ? "pointer" : "default" }}><text>{title}</text></div></Fragment> })}
    </div>
  </div>
}

function RecipeComposer(props: {
  recipes?: readonly CapsuleRecipe[]
  loading?: boolean
  error?: string
  selectedRecipeId?: string
  onSelectRecipe?(recipeId?: string): void
}) {
  const [open, setOpen] = useState(false)
  const recipes = props.recipes ?? []
  const selected = recipes.find((recipe) => recipe.id === props.selectedRecipeId)
  const unavailableSelection = props.selectedRecipeId !== undefined && selected === undefined && !props.loading
  const canSelect = Boolean(props.onSelectRecipe)
  const canClear = props.selectedRecipeId !== undefined && canSelect
  return <div testId="recipe-composer" style={{ display: "flex", flexDirection: "column", gap: 5, padding: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 8, backgroundColor: palette.panel }}>
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <text style={{ color: palette.textMuted, fontSize: 10 }}>Recipe</text>
      <div testId="recipe-picker" tabIndex={canSelect ? 0 : -1} accessibilityRole="button" accessibilityName="Choose run recipe" accessibilityExpanded={open} accessibilityDisabled={!canSelect} onClick={() => { if (canSelect) setOpen((value) => !value) }} onKeyDown={(event) => { if (activates(event) && canSelect) setOpen((value) => !value) }} style={{ flexGrow: 1, minWidth: 0, cursor: canSelect ? "pointer" : "default", padding: 5, backgroundColor: palette.panelRaised }}><text style={{ color: selected ? palette.text : unavailableSelection ? "#f0b060" : palette.textMuted, fontSize: 11 }}>{selected?.name ?? (unavailableSelection ? `Unavailable recipe: ${props.selectedRecipeId}` : "No recipe")}</text></div>
      <div testId="clear-recipe" tabIndex={canClear ? 0 : -1} accessibilityRole="button" accessibilityName="Clear recipe" accessibilityDisabled={!canClear} onClick={() => { if (canClear) props.onSelectRecipe?.(undefined) }} onKeyDown={(event) => { if (activates(event) && canClear) props.onSelectRecipe?.(undefined) }} style={{ cursor: canClear ? "pointer" : "default", padding: 5, backgroundColor: canClear ? palette.panelRaised : palette.panel, color: canClear ? palette.textMuted : palette.textFaint }}><text>Clear</text></div>
    </div>
    {open && <div testId="recipe-options" accessibilityRole="listbox" accessibilityName="Run recipes" style={{ display: "flex", flexDirection: "column", gap: 3 }}>{recipes.map((recipe) => <Fragment key={recipe.id}><div testId={`recipe-option-${recipe.id}`} tabIndex={0} accessibilityRole="option" accessibilityName={recipe.name} accessibilitySelected={recipe.id === props.selectedRecipeId} onClick={() => { props.onSelectRecipe?.(recipe.id); setOpen(false) }} onKeyDown={(event) => { if (activates(event)) { props.onSelectRecipe?.(recipe.id); setOpen(false) } }} style={{ cursor: "pointer", padding: 5, backgroundColor: recipe.id === props.selectedRecipeId ? palette.accentDim : palette.panelRaised }}><text style={{ color: palette.text, fontSize: 11 }}>{recipe.name}</text></div></Fragment>)}</div>}
    {props.loading && <text testId="recipe-loading" style={{ color: palette.textFaint, fontSize: 10 }}>Loading recipes...</text>}
    {props.error && <text testId="recipe-error" accessibilityRole="alert" accessibilityName={props.error} style={{ color: "#f08080", fontSize: 10 }}>{props.error}</text>}
    {unavailableSelection && <text testId="recipe-unavailable" accessibilityRole="alert" accessibilityName={`Selected recipe ${props.selectedRecipeId} is unavailable`} style={{ color: "#f0b060", fontSize: 10 }}>The selected recipe is unavailable. Clear it or choose a listed recipe.</text>}
    {selected && <div testId="selected-recipe" style={{ display: "flex", flexDirection: "column", gap: 2 }}><text style={{ color: palette.text, fontSize: 11 }}>{selected.name}</text>{selected.description && <text style={{ color: palette.textMuted, fontSize: 10 }}>{selected.description}</text>}<text testId="selected-recipe-contract" style={{ color: palette.textFaint, fontSize: 10 }}>Contract: {selected.contract}</text></div>}
  </div>
}

type TranscriptViewRow =
  | { kind: "durable"; key: string; row: AgentTurnRow }
  | { kind: "liveAssistant"; key: string; message: AssistantMessage }

const Transcript = memo(function Transcript(props: {
  sessionId?: SessionId
  rows: readonly AgentTurnRow[]
  liveMessages: readonly AssistantMessage[]
  loading: boolean
  error?: string
  hasOlder: boolean
  loadingOlder: boolean
  onLoadOlder(): void
  hiddenRunIds?: ReadonlySet<string>
  onOpenSideChat?(parentRunId: string, parentEventSeq: string): void
  onOpenFreshSpawn?(parentRunId: string): void
  onPinThreadWorkspace?(runId: string, cursor: string): void
}) {
  const [followTail, setFollowTail] = useState(true)
  useEffect(() => setFollowTail(true), [props.sessionId])

  const rows = useMemo<TranscriptViewRow[]>(() => {
    const committedAssistantIds = new Set(props.rows.flatMap((row) => (
      row.kind === "assistant" ? [String(row.turnId ?? row.runId)] : []
    )))
    const durable = props.rows.filter((row) => !props.hiddenRunIds?.has(row.runId)).map((row) => ({
      kind: "durable" as const,
      key: `durable-${row.cursor.sequence}-${row.kind}`,
      row,
    }))
    const live = props.liveMessages
      .filter((message) => !committedAssistantIds.has(message.id))
      .map((message) => ({ kind: "liveAssistant" as const, key: `live-${message.id}`, message }))
    return [...durable, ...live]
  }, [props.hiddenRunIds, props.liveMessages, props.rows])

  const loadOlderOffset = props.hasOlder ? 1 : 0
  const itemCount = rows.length + loadOlderOffset
  const renderItem = useCallback((index: number) => {
    if (props.hasOlder && index === 0) {
      return <Fragment key="load-older"><div testId="load-older-transcript" tabIndex={props.loadingOlder ? -1 : 0} accessibilityRole="button" accessibilityName="Load earlier messages" accessibilityDisabled={props.loadingOlder} onClick={() => { if (!props.loadingOlder) props.onLoadOlder() }} onKeyDown={(event) => { if (activates(event) && !props.loadingOlder) props.onLoadOlder() }} style={{ display: "flex", justifyContent: "center", padding: 12, cursor: props.loadingOlder ? "default" : "pointer" }}>
        <text style={{ color: palette.textMuted, fontSize: 11 }}>{props.loadingOlder ? "Loading earlier messages..." : "Load earlier messages"}</text>
      </div></Fragment>
    }
    const item = rows[index - loadOlderOffset]
    return item ? <TranscriptRow key={item.key} item={item} onOpenSideChat={props.onOpenSideChat} onOpenFreshSpawn={props.onOpenFreshSpawn} onPinThreadWorkspace={props.onPinThreadWorkspace} /> : null
  }, [loadOlderOffset, props.hasOlder, props.loadingOlder, props.onLoadOlder, props.onOpenFreshSpawn, props.onOpenSideChat, props.onPinThreadWorkspace, rows])

  if (!props.sessionId) return <div testId="conversation" style={{ display: "flex", flexGrow: 1, alignItems: "center", justifyContent: "center" }}><text style={{ color: palette.textMuted }}>Select a conversation to see its transcript.</text></div>
  if (props.loading && !rows.length) return <div testId="conversation" style={{ display: "flex", flexGrow: 1, alignItems: "center", justifyContent: "center" }}><text style={{ color: palette.textMuted }}>Loading conversation...</text></div>
  if (props.error && !rows.length) return <div testId="conversation" style={{ display: "flex", flexGrow: 1, alignItems: "center", justifyContent: "center" }}><text style={{ color: "#f08080" }}>{props.error}</text></div>
  if (!rows.length) return <div testId="conversation" style={{ display: "flex", flexGrow: 1, alignItems: "center", justifyContent: "center" }}><text testId="conversation-placeholder" style={{ color: palette.textMuted, fontSize: 13, userSelect: "text" }}>Start a run to begin this conversation.</text></div>

  return <div testId="conversation" style={{ display: "flex", flexDirection: "column", flexGrow: 1, minHeight: 0, position: "relative" }}>
    <VirtualList
      itemCount={itemCount}
      renderItem={renderItem}
      estimatedItemHeight={132}
      overdraw={264}
      alignment="bottom"
      followTail={followTail}
      onVisibleRange={(event) => {
        const end = Number((event as { endIndex?: number | null }).endIndex ?? 0)
        setFollowTail(end >= itemCount)
      }}
      style={{ flexGrow: 1, minHeight: 0, width: "100%" }}
    />
    {!followTail && <div testId="jump-to-latest" tabIndex={0} accessibilityRole="button" accessibilityName="Jump to latest" accessibilityDisabled={false} onClick={() => setFollowTail(true)} onKeyDown={(event) => { if (activates(event)) setFollowTail(true) }} style={{ position: "absolute", right: 14, bottom: 10, padding: 7, borderRadius: 7, backgroundColor: palette.panelRaised, cursor: "pointer" }}><text style={{ color: palette.text, fontSize: 11 }}>Jump to latest</text></div>}
  </div>
})

function TranscriptRow({ item, onOpenSideChat, onOpenFreshSpawn, onPinThreadWorkspace }: { item: TranscriptViewRow; onOpenSideChat?(parentRunId: string, parentEventSeq: string): void; onOpenFreshSpawn?(parentRunId: string): void; onPinThreadWorkspace?(runId: string, cursor: string): void }) {
  if (item.kind === "liveAssistant") {
    return <div testId={`assistant-message-${item.message.id}`} style={{ display: "flex", flexDirection: "column", gap: 6, paddingTop: 12, paddingBottom: 12, paddingLeft: 8, paddingRight: 8 }}>
      <text style={{ color: palette.accent, fontSize: 10 }}>ASSISTANT · STREAMING</text>
      <markdown source={item.message.text} />
    </div>
  }
  const row = item.row
  const sideChat = onOpenSideChat
    ? <div testId={`side-chat-${row.cursor.sequence}`} tabIndex={0} accessibilityRole="button" accessibilityName="Open side chat" accessibilityDisabled={false} onClick={() => onOpenSideChat(row.runId, row.cursor.sequence)} onKeyDown={(event) => { if (activates(event)) onOpenSideChat(row.runId, row.cursor.sequence) }} style={{ cursor: "pointer", padding: 6, borderRadius: 6, backgroundColor: palette.panelRaised }}><text style={{ color: palette.textMuted, fontSize: 10 }}>Side Chat</text></div>
    : null
  const freshSpawn = onOpenFreshSpawn
    ? <div testId={`fresh-spawn-${row.cursor.sequence}`} tabIndex={0} accessibilityRole="button" accessibilityName="Create fresh spawn" accessibilityDisabled={false} onClick={() => onOpenFreshSpawn(row.runId)} onKeyDown={(event) => { if (activates(event)) onOpenFreshSpawn(row.runId) }} style={{ cursor: "pointer", padding: 6, borderRadius: 6, backgroundColor: palette.panelRaised }}><text style={{ color: palette.textMuted, fontSize: 10 }}>Fresh Spawn</text></div>
    : null
  const pin = onPinThreadWorkspace
    ? <div testId={`pin-thread-workspace-${row.cursor.sequence}`} tabIndex={0} accessibilityRole="button" accessibilityName="Pin to thread workspace" accessibilityDisabled={false} onClick={() => onPinThreadWorkspace(row.runId, row.cursor.sequence)} onKeyDown={(event) => { if (activates(event)) onPinThreadWorkspace(row.runId, row.cursor.sequence) }} style={{ cursor: "pointer", padding: 6, borderRadius: 6, backgroundColor: palette.panelRaised }}><text style={{ color: palette.textMuted, fontSize: 10 }}>Pin</text></div>
    : null
  if (row.kind === "user") {
    return <div testId={`user-message-${row.cursor.sequence}`} style={{ display: "flex", justifyContent: "flex-end", gap: 8, paddingTop: 12, paddingBottom: 12, paddingLeft: 56, paddingRight: 8 }}>
      <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 760, padding: 12, borderRadius: 12, backgroundColor: palette.panelRaised }}>
        <text style={{ color: palette.text, fontSize: 13, userSelect: "text" }}>{row.text}</text>
        {!!row.attachments.length && <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>{row.attachments.map((attachment) => <Fragment key={`${attachment.path}:${attachment.revision}`}><div style={{ padding: 6, borderRadius: 6, borderWidth: 1, borderColor: palette.border }}><text style={{ color: palette.textMuted, fontSize: 10, userSelect: "text" }}>{attachment.kind === "image" ? `Image · ${attachment.path}` : attachment.path}</text></div></Fragment>)}</div>}
      </div>
      {sideChat}{freshSpawn}{pin}
    </div>
  }
  if (row.kind === "assistant") {
    return <div testId={`assistant-message-${row.cursor.sequence}`} style={{ display: "flex", flexDirection: "column", gap: 6, paddingTop: 12, paddingBottom: 12, paddingLeft: 8, paddingRight: 8 }}>
      <text style={{ color: palette.textFaint, fontSize: 10 }}>ASSISTANT</text>
      <markdown source={row.text} />
      {sideChat}{freshSpawn}{pin}
    </div>
  }
  if (row.kind === "toolCall") {
    const outputIsDiff = isUnifiedDiff(row.output)
    return <div testId={`tool-call-${row.cursor.sequence}`} style={{ display: "flex", flexDirection: "column", gap: 8, marginTop: 8, marginBottom: 8, padding: 12, borderWidth: 1, borderColor: palette.border, borderRadius: 9, backgroundColor: palette.panel }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}><text style={{ color: palette.text, fontSize: 12, fontWeight: 600 }}>{row.toolName}</text><text style={{ color: palette.textFaint, fontSize: 10 }}>{String(row.outcome).toUpperCase()}</text></div>
      {row.input && <code code={row.input} language="json" showHeader={false} />}
      {row.output && (outputIsDiff
        ? <diff patch={row.output} wordDiff maxLines={240} />
        : <code code={row.output} showHeader={false} />)}
      {sideChat}{freshSpawn}{pin}
    </div>
  }
  return <div testId={`pending-state-${row.cursor.sequence}`} style={{ display: "flex", justifyContent: "center", gap: 8, padding: 8 }}>
    <text style={{ color: palette.warning, fontSize: 10 }}>{pendingStateLabel(row.state)}</text>
    {sideChat}{freshSpawn}{pin}
  </div>
}

function FreshSpawnComposer(props: { parentRunId: string; onDismiss(): void; onSpawn?(parentRunId: string, objective: string): Promise<void> }) {
  const [draft, setDraft] = useState("")
  const enabled = Boolean(props.onSpawn && draft.trim())
  return <div testId={`fresh-spawn-composer-${props.parentRunId}`} style={{ display: "flex", flexDirection: "column", gap: 7, padding: 10, borderWidth: 1, borderColor: palette.accentDim, borderRadius: 8, backgroundColor: palette.panel }}>
    <text style={{ color: palette.textMuted, fontSize: 10 }}>Fresh Spawn from {props.parentRunId}</text>
    <input testId={`fresh-spawn-objective-${props.parentRunId}`} value={draft} onChange={(event) => setDraft(event.value ?? "")} placeholder="Independent task with fresh context" />
    <div style={{ display: "flex", gap: 6 }}>
      <div testId={`spawn-fresh-run-${props.parentRunId}`} tabIndex={enabled ? 0 : -1} accessibilityRole="button" accessibilityName="Spawn fresh run" accessibilityDisabled={!enabled} onClick={() => { const objective = draft.trim(); if (!objective || !props.onSpawn) return; void props.onSpawn(props.parentRunId, objective).then(() => { setDraft(""); props.onDismiss() }) }} onKeyDown={(event) => { if (activates(event)) { const objective = draft.trim(); if (!objective || !props.onSpawn) return; void props.onSpawn(props.parentRunId, objective).then(() => { setDraft(""); props.onDismiss() }) } }} style={{ cursor: enabled ? "pointer" : "default", padding: 6, backgroundColor: enabled ? palette.accentDim : palette.panelRaised }}><text style={{ color: enabled ? palette.text : palette.textFaint, fontSize: 10 }}>Spawn</text></div>
      <div testId={`dismiss-fresh-spawn-${props.parentRunId}`} tabIndex={0} accessibilityRole="button" accessibilityName="Cancel fresh spawn" accessibilityDisabled={false} onClick={props.onDismiss} onKeyDown={(event) => { if (activates(event)) props.onDismiss() }} style={{ cursor: "pointer", padding: 6, backgroundColor: palette.panelRaised }}><text style={{ color: palette.textMuted, fontSize: 10 }}>Cancel</text></div>
    </div>
  </div>
}

function FreshSpawnPanel(props: { run: RunListEntry; onJoin?(parentRunId: string, childRunId: string): Promise<JoinRunResult | undefined> }) {
  const [joined, setJoined] = useState<JoinRunResult>()
  if (props.run.relationship.kind !== "freshSpawn") return null
  const parentRunId = props.run.relationship.parentRunId
  return <div testId={`fresh-spawn-panel-${props.run.id}`} style={{ display: "flex", flexDirection: "column", gap: 6, padding: 10, borderWidth: 1, borderColor: palette.border, borderRadius: 8, backgroundColor: palette.panel }}>
    <text style={{ color: palette.text, fontSize: 11, fontWeight: 650 }}>Fresh Spawn</text>
    <text testId={`fresh-spawn-status-${props.run.id}`} style={{ color: palette.textMuted, fontSize: 10 }}>Status: {runStatusLabel(props.run.status)}</text>
    <div testId={`join-fresh-run-${props.run.id}`} tabIndex={props.onJoin ? 0 : -1} accessibilityRole="button" accessibilityName="Join fresh run" accessibilityDisabled={!props.onJoin} onClick={() => { if (!props.onJoin) return; void props.onJoin(parentRunId, props.run.id).then((result) => { if (result) setJoined(result) }) }} onKeyDown={(event) => { if (activates(event) && props.onJoin) void props.onJoin(parentRunId, props.run.id).then((result) => { if (result) setJoined(result) }) }} style={{ cursor: props.onJoin ? "pointer" : "default", padding: 6, backgroundColor: props.onJoin ? palette.accentDim : palette.panelRaised }}><text style={{ color: props.onJoin ? palette.text : palette.textFaint, fontSize: 10 }}>Join</text></div>
    {joined && <div testId={`fresh-join-links-${props.run.id}`} style={{ display: "flex", flexDirection: "column", gap: 3 }}>
      <text testId={`fresh-join-status-${props.run.id}`} style={{ color: palette.textMuted, fontSize: 10 }}>Daemon status: {runStatusLabel(joined.run.status)}</text>
      <text testId={`fresh-join-result-${props.run.id}`} style={{ color: palette.textMuted, fontSize: 10 }}>{joined.result ? "Result available" : "Result pending"}</text>
      {(joined.receipts ?? []).map((receipt) => <Fragment key={receipt.id}><text testId={`fresh-join-receipt-${receipt.id}`} style={{ color: palette.textMuted, fontSize: 10 }}>Receipt: {receipt.id}</text></Fragment>)}
      {(joined.artifacts ?? []).map((artifact) => <Fragment key={artifact.id}><text testId={`fresh-join-artifact-${artifact.id}`} style={{ color: palette.textMuted, fontSize: 10 }}>Artifact: {artifact.displayName}</text></Fragment>)}
    </div>}
  </div>
}

function SideChatPanel(props: { run: RunListEntry; rows: readonly AgentTurnRow[]; onCancel?(runId: string): void; onClose?(runId: string): void; onContinue?(runId: string, message: string): void }) {
  const canCancel = props.run.status === "queued" || props.run.status === "running" || props.run.status === "waitingForApproval"
  const canContinue = props.run.status === "completed" || props.run.status === "failed" || props.run.status === "cancelled" || props.run.status === "budgetExceeded"
  const [draft, setDraft] = useState("")
  const continuation = props.onContinue && draft.trim() ? { message: draft.trim(), send: props.onContinue } : undefined
  const sendContinuation = () => {
    if (!continuation) return
    continuation.send(props.run.id, continuation.message)
    setDraft("")
  }
  return <div testId={`side-chat-panel-${props.run.id}`} style={{ display: "flex", flexDirection: "column", gap: 8, padding: 12, borderWidth: 1, borderColor: palette.accentDim, borderRadius: 9, backgroundColor: palette.panel }}>
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}><text style={{ color: palette.text, fontSize: 13, fontWeight: 650 }}>Side Chat</text><div style={{ flexGrow: 1 }} /><text testId={`side-chat-status-${props.run.id}`} style={{ color: palette.textMuted, fontSize: 10 }}>{runStatusLabel(props.run.status)}</text></div>
    {props.run.relationship.kind === "fork" && <text testId={`side-chat-lineage-${props.run.id}`} style={{ color: palette.textMuted, fontSize: 10, userSelect: "text" }}>From {props.run.relationship.parentRunId} at turn {props.run.relationship.parentEventSeq}</text>}
    {!props.rows.length && <text style={{ color: palette.textFaint, fontSize: 11 }}>Waiting for the daemon-projected child.</text>}
    {props.rows.slice(-3).map((row) => <Fragment key={`${row.cursor.sequence}-${row.kind}`}><text style={{ color: palette.textMuted, fontSize: 11 }}>[{row.kind}]</text></Fragment>)}
    {canContinue && <div style={{ display: "flex", gap: 8 }}>
      <input testId={`continue-side-chat-input-${props.run.id}`} value={draft} onChange={(event) => setDraft(event.value ?? "")} placeholder="Continue this side chat" style={{ flexGrow: 1, minWidth: 0 }} />
      <div testId={`continue-side-chat-${props.run.id}`} tabIndex={continuation ? 0 : -1} accessibilityRole="button" accessibilityName="Send side chat message" accessibilityDisabled={!continuation} onClick={sendContinuation} onKeyDown={(event) => { if (activates(event)) sendContinuation() }} style={{ cursor: continuation ? "pointer" : "default", padding: 6, backgroundColor: continuation ? palette.accentDim : palette.panelRaised }}><text style={{ color: continuation ? palette.text : palette.textFaint, fontSize: 10 }}>Send</text></div>
    </div>}
    <div style={{ display: "flex", gap: 8 }}>
      <div testId={`cancel-side-chat-${props.run.id}`} tabIndex={canCancel && props.onCancel ? 0 : -1} accessibilityRole="button" accessibilityName="Cancel side chat" accessibilityDisabled={!canCancel || !props.onCancel} onClick={() => { if (canCancel && props.onCancel) props.onCancel(props.run.id) }} onKeyDown={(event) => { if (activates(event) && canCancel && props.onCancel) props.onCancel(props.run.id) }} style={{ cursor: canCancel && props.onCancel ? "pointer" : "default", padding: 6, backgroundColor: canCancel && props.onCancel ? palette.accentDim : palette.panelRaised }}><text style={{ color: canCancel && props.onCancel ? palette.text : palette.textFaint, fontSize: 10 }}>Cancel</text></div>
      <div testId={`close-side-chat-${props.run.id}`} tabIndex={props.onClose ? 0 : -1} accessibilityRole="button" accessibilityName="Close side chat" accessibilityDisabled={!props.onClose} onClick={() => { if (props.onClose) props.onClose(props.run.id) }} onKeyDown={(event) => { if (activates(event) && props.onClose) props.onClose(props.run.id) }} style={{ cursor: props.onClose ? "pointer" : "default", padding: 6, backgroundColor: palette.panelRaised }}><text style={{ color: props.onClose ? palette.textMuted : palette.textFaint, fontSize: 10 }}>Close</text></div>
    </div>
  </div>
}

function isUnifiedDiff(value: string): boolean {
  return /^(?:diff --git |--- )/m.test(value) && /^\+\+\+ /m.test(value) && /^@@ /m.test(value)
}

function pendingStateLabel(state: RuntimeLanePendingState): string {
  return state.replace(/([A-Z])/g, " $1").trim().toUpperCase()
}

function runStatusLabel(status: RunStatus): string {
  if (status === "waitingForApproval") return "WAITING FOR APPROVAL"
  if (status === "budgetExceeded") return "BUDGET EXCEEDED"
  return status.toUpperCase()
}
