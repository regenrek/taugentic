import type { NavigationConversation, NavigationSnapshot, ProjectId, SessionId } from "@taugentic/desktop-protocol"
import { Fragment, type ReactNode } from "react"

import { palette } from "../../app/theme.js"

function activates(event: { key?: string }): boolean { return event.key === "enter" || event.key === "space" }

export type SidebarView = "spaces" | "projects" | "agents" | "archived"

export type SidebarState = {
  view: SidebarView
  filter: string
  expandedSpaceIds: readonly string[]
  selectedConversationId?: SessionId
}

export type SidebarAction =
  | { type: "selectView"; view: SidebarView }
  | { type: "selectConversation"; sessionId: SessionId }
  | { type: "selectProject"; projectId: ProjectId }
  | { type: "openProject" }
  | { type: "setFilter"; filter: string }
  | { type: "toggleSpace"; spaceId: string }

export function sidebarReduce(state: SidebarState, action: SidebarAction): SidebarState {
  if (action.type === "selectView") return { ...state, view: action.view }
  if (action.type === "selectConversation") return state
  if (action.type === "selectProject" || action.type === "openProject") return state
  if (action.type === "setFilter") return { ...state, filter: action.filter }
  const expanded = state.expandedSpaceIds.includes(action.spaceId)
    ? state.expandedSpaceIds.filter((id) => id !== action.spaceId)
    : [...state.expandedSpaceIds, action.spaceId]
  return { ...state, expandedSpaceIds: expanded }
}

export function projectConversations(snapshot: NavigationSnapshot, projectId?: ProjectId): NavigationConversation[] {
  if (!projectId) return []
  return (snapshot.conversations ?? []).filter((conversation) => (
    !conversation.archived
    && conversation.placement.kind === "project"
    && conversation.placement.projectId === projectId
  )).sort((left, right) => Number(right.pinned) - Number(left.pinned))
}

export function archivedProjectConversations(snapshot: NavigationSnapshot, projectId?: ProjectId): NavigationConversation[] {
  if (!projectId) return []
  return (snapshot.conversations ?? []).filter((conversation) => (
    conversation.archived
    && conversation.placement.kind === "project"
    && conversation.placement.projectId === projectId
  ))
}

export function standaloneConversations(snapshot: NavigationSnapshot): NavigationConversation[] {
  return (snapshot.conversations ?? []).filter((conversation) => (
    !conversation.archived && conversation.placement.kind === "standalone"
  )).sort((left, right) => Number(right.pinned) - Number(left.pinned))
}

export function temporaryConversations(snapshot: NavigationSnapshot): NavigationConversation[] {
  return (snapshot.conversations ?? []).filter((conversation) => (
    !conversation.archived && conversation.placement.kind === "temporary"
  )).sort((left, right) => Number(right.pinned) - Number(left.pinned))
}

type SidebarProps = {
  snapshot: NavigationSnapshot
  state: SidebarState
  selectedProjectId?: ProjectId
  spaceTitle?: string
  conversationTitle: string
  standaloneTitle?: string
  temporaryTitle?: string
  canCreateSpace?: boolean
  canCreateConversation: boolean
  canCreateStandalone?: boolean
  canCreateTemporary?: boolean
  canOrganizeConversations?: boolean
  canOrganizeProjects?: boolean
  searchMode?: boolean
  searchLoading?: boolean
  searchError?: boolean
  workInbox?: ReactNode
  scheduledWork?: ReactNode
  dispatch(action: SidebarAction): void
  onSpaceTitleChange?(value: string): void
  onCreateSpace?(): void
  onSetProjectSpace?(projectId: ProjectId, spaceId?: string): void
  onConversationTitleChange(value: string): void
  onStandaloneTitleChange?(value: string): void
  onTemporaryTitleChange?(value: string): void
  onCreateConversation(): void
  onCreateStandalone?(): void
  onCreateTemporary?(): void
  onSetPinnedConversation?(sessionId: SessionId, pinned: boolean): void
  onArchiveConversation?(sessionId: SessionId): void
  onRestoreConversation?(sessionId: SessionId): void
  onCloseTemporaryConversation?(sessionId: SessionId): void
}

export function Sidebar({ snapshot, state, selectedProjectId, spaceTitle = "", conversationTitle, standaloneTitle = "", temporaryTitle = "", canCreateSpace = false, canCreateConversation, canCreateStandalone = false, canCreateTemporary = false, canOrganizeConversations = false, canOrganizeProjects = false, searchMode = false, searchLoading = false, searchError = false, workInbox, scheduledWork, dispatch, onSpaceTitleChange, onCreateSpace, onSetProjectSpace, onConversationTitleChange, onStandaloneTitleChange, onTemporaryTitleChange, onCreateConversation, onCreateStandalone, onCreateTemporary, onSetPinnedConversation, onArchiveConversation, onRestoreConversation, onCloseTemporaryConversation }: SidebarProps) {
  const activeConversations = projectConversations(snapshot, selectedProjectId)
  const archivedConversations = archivedProjectConversations(snapshot, selectedProjectId)
  const standalone = standaloneConversations(snapshot)
  const temporary = temporaryConversations(snapshot)
  const conversations = searchMode
    ? snapshot.conversations ?? []
    : state.view === "archived" ? archivedConversations : [...activeConversations, ...standalone, ...temporary]
  const projects = snapshot.projects ?? []
  const spaces = snapshot.spaces ?? []

  return <div testId="workspace-sidebar" style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, overflow: "scroll", padding: 14, gap: 8, backgroundColor: palette.panel, minWidth: 220 }}>
    <text style={{ color: palette.textFaint, fontSize: 10, fontWeight: 700 }}>WORKSPACE</text>
    <input testId="sidebar-filter" value={state.filter} placeholder="Filter conversations" onChange={(event) => dispatch({ type: "setFilter", filter: event.value ?? "" })} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
    <div style={{ display: "flex", gap: 6 }}>
      {(["spaces", "projects", "agents", "archived"] as const).map((view) => <Fragment key={view}><div testId={`sidebar-view-${view}`} tabIndex={0} accessibilityRole="tab" accessibilityName={`${view} view`} accessibilitySelected={state.view === view} onClick={() => dispatch({ type: "selectView", view })} onKeyDown={(event) => { if (activates(event)) dispatch({ type: "selectView", view }) }} style={{ cursor: "pointer", padding: 6, backgroundColor: state.view === view ? palette.panelRaised : palette.panel }}><text style={{ color: palette.textMuted, fontSize: 11 }}>{view}</text></div></Fragment>)}
    </div>
    {!searchMode && state.view === "spaces" && <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <input testId="new-space-title" value={spaceTitle} placeholder="Space name" onChange={(event) => onSpaceTitleChange?.(event.value ?? "")} onSubmit={() => { if (canCreateSpace) onCreateSpace?.() }} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
      <div testId="create-space" tabIndex={canCreateSpace ? 0 : -1} accessibilityRole="button" accessibilityName="Create space" accessibilityDisabled={!canCreateSpace} onClick={() => { if (canCreateSpace) onCreateSpace?.() }} onKeyDown={(event) => { if (activates(event) && canCreateSpace) onCreateSpace?.() }} style={{ cursor: canCreateSpace ? "pointer" : "default", padding: 8, backgroundColor: canCreateSpace ? palette.accentDim : palette.panelRaised }}><text>Create space</text></div>
    </div>}
    {!searchMode && state.view === "spaces" && spaces.map((space) => <Fragment key={space.id}><div>
      <div testId={`space-${space.id}`} tabIndex={0} accessibilityRole="button" accessibilityName={`${space.title} space`} accessibilityExpanded={state.expandedSpaceIds.includes(space.id)} onClick={() => dispatch({ type: "toggleSpace", spaceId: space.id })} onKeyDown={(event) => { if (activates(event)) dispatch({ type: "toggleSpace", spaceId: space.id }) }} style={{ cursor: "pointer", padding: 7 }}><text>{state.expandedSpaceIds.includes(space.id) ? "⌄ " : "› "}{space.title}</text></div>
      {state.expandedSpaceIds.includes(space.id) && projects.filter((project) => project.spaceId === space.id).map((project) => <Fragment key={project.id}><div testId={`project-${project.id}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Open project ${project.title}`} accessibilitySelected={selectedProjectId === project.id} onClick={() => dispatch({ type: "selectProject", projectId: project.id })} onKeyDown={(event) => { if (activates(event)) dispatch({ type: "selectProject", projectId: project.id }) }} style={{ cursor: "pointer", padding: 7, marginLeft: 16, backgroundColor: selectedProjectId === project.id ? palette.panelRaised : palette.panel }}><text style={{ color: palette.textMuted, fontSize: 12 }}>{project.title}</text></div></Fragment>)}
    </div></Fragment>)}
    {!searchMode && state.view === "projects" && projects.map((project) => <Fragment key={project.id}><div testId={`project-${project.id}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Open project ${project.title}`} accessibilitySelected={selectedProjectId === project.id} onClick={() => dispatch({ type: "selectProject", projectId: project.id })} onKeyDown={(event) => { if (activates(event)) dispatch({ type: "selectProject", projectId: project.id }) }} style={{ cursor: "pointer", padding: 7, backgroundColor: selectedProjectId === project.id ? palette.panelRaised : palette.panel }}><text style={{ color: palette.textMuted, fontSize: 12 }}>{project.title}</text></div>{selectedProjectId === project.id && <div testId={`project-space-controls-${project.id}`} style={{ display: "flex", flexDirection: "column", gap: 4, paddingLeft: 12 }}><text style={{ color: palette.textFaint, fontSize: 10, fontWeight: 700 }}>SPACE</text><ProjectSpaceAction testId={`set-project-space-${project.id}-ungrouped`} label="Ungrouped" selected={!project.spaceId} enabled={canOrganizeProjects} onActivate={() => onSetProjectSpace?.(project.id)} />{spaces.map((space) => <ProjectSpaceAction key={space.id} testId={`set-project-space-${project.id}-${space.id}`} label={space.title} selected={project.spaceId === space.id} enabled={canOrganizeProjects} onActivate={() => onSetProjectSpace?.(project.id, space.id)} />)}</div>}</Fragment>)}
    {!searchMode && state.view === "projects" && <div testId="sidebar-open-project" tabIndex={0} accessibilityRole="button" accessibilityName="Open project" onClick={() => dispatch({ type: "openProject" })} onKeyDown={(event) => { if (activates(event)) dispatch({ type: "openProject" }) }} style={{ cursor: "pointer", padding: 8, backgroundColor: palette.panelRaised }}><text>Open project</text></div>}
    {!searchMode && state.view === "agents" && (snapshot.agents ?? []).map((agent) => <Fragment key={agent.sessionId}><text style={{ color: palette.textMuted, fontSize: 12, padding: 7 }}>{agent.title}</text></Fragment>)}
    {!searchMode && workInbox}
    {!searchMode && scheduledWork}
    <text testId={searchMode ? "sidebar-search-results" : undefined} style={{ color: palette.textFaint, fontSize: 10, fontWeight: 700, marginTop: 8 }}>{searchMode ? "SEARCH RESULTS" : "CONVERSATIONS"}</text>
    {!searchMode && selectedProjectId && state.view !== "archived" && <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <input testId="new-conversation-title" value={conversationTitle} placeholder="Conversation title" onChange={(event) => onConversationTitleChange(event.value ?? "")} onSubmit={() => { if (canCreateConversation) onCreateConversation() }} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
      <div testId="create-conversation" tabIndex={canCreateConversation ? 0 : -1} accessibilityRole="button" accessibilityName="New conversation" accessibilityDisabled={!canCreateConversation} onClick={() => { if (canCreateConversation) onCreateConversation() }} onKeyDown={(event) => { if (activates(event) && canCreateConversation) onCreateConversation() }} style={{ cursor: canCreateConversation ? "pointer" : "default", padding: 8, backgroundColor: canCreateConversation ? palette.accentDim : palette.panelRaised }}><text>New conversation</text></div>
    </div>}
    {!searchMode && state.view !== "archived" && <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <input testId="new-standalone-title" value={standaloneTitle} placeholder="Standalone conversation" onChange={(event) => onStandaloneTitleChange?.(event.value ?? "")} onSubmit={() => { if (canCreateStandalone) onCreateStandalone?.() }} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
      <div testId="create-standalone-conversation" tabIndex={canCreateStandalone ? 0 : -1} accessibilityRole="button" accessibilityName="New standalone conversation" accessibilityDisabled={!canCreateStandalone} onClick={() => { if (canCreateStandalone) onCreateStandalone?.() }} onKeyDown={(event) => { if (activates(event) && canCreateStandalone) onCreateStandalone?.() }} style={{ cursor: canCreateStandalone ? "pointer" : "default", padding: 8, backgroundColor: canCreateStandalone ? palette.accentDim : palette.panelRaised }}><text>New standalone</text></div>
      <input testId="new-temporary-title" value={temporaryTitle} placeholder="Temporary conversation" onChange={(event) => onTemporaryTitleChange?.(event.value ?? "")} onSubmit={() => { if (canCreateTemporary) onCreateTemporary?.() }} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
      <div testId="create-temporary-conversation" tabIndex={canCreateTemporary ? 0 : -1} accessibilityRole="button" accessibilityName="New temporary conversation" accessibilityDisabled={!canCreateTemporary} onClick={() => { if (canCreateTemporary) onCreateTemporary?.() }} onKeyDown={(event) => { if (activates(event) && canCreateTemporary) onCreateTemporary?.() }} style={{ cursor: canCreateTemporary ? "pointer" : "default", padding: 8, backgroundColor: canCreateTemporary ? palette.accentDim : palette.panelRaised }}><text>New temporary</text></div>
    </div>}
    {searchLoading && <text testId="sidebar-search-loading" style={{ color: palette.textMuted, fontSize: 12 }}>Searching conversations…</text>}
    {searchError && <text testId="sidebar-search-error" accessibilityRole="alert" style={{ color: palette.warning, fontSize: 12 }}>Search results could not be loaded.</text>}
    {!searchLoading && !searchError && conversations.map((conversation) => <ConversationRow key={conversation.sessionId} conversation={conversation} archived={conversation.archived} selected={state.selectedConversationId === conversation.sessionId} canOrganize={canOrganizeConversations} onOpen={() => dispatch({ type: "selectConversation", sessionId: conversation.sessionId })} onSetPinned={(pinned) => onSetPinnedConversation?.(conversation.sessionId, pinned)} onArchive={() => onArchiveConversation?.(conversation.sessionId)} onRestore={() => onRestoreConversation?.(conversation.sessionId)} onCloseTemporary={() => onCloseTemporaryConversation?.(conversation.sessionId)} />)}
    {!searchLoading && !searchError && !conversations.length && <text style={{ color: palette.textMuted, fontSize: 12 }}>{searchMode ? "No matching conversations." : state.view === "archived" ? "No archived conversations available." : "No conversations available."}</text>}
  </div>
}

function ProjectSpaceAction({ testId, label, selected, enabled, onActivate }: { testId: string; label: string; selected: boolean; enabled: boolean; onActivate(): void }) {
  return <div testId={testId} tabIndex={enabled ? 0 : -1} accessibilityRole="button" accessibilityName={`Move project to ${label}`} accessibilitySelected={selected} accessibilityDisabled={!enabled} onClick={() => { if (enabled) onActivate() }} onKeyDown={(event) => { if (activates(event) && enabled) onActivate() }} style={{ cursor: enabled ? "pointer" : "default", padding: 5, backgroundColor: selected ? palette.accentDim : palette.panelRaised }}><text>{label}</text></div>
}

function ConversationRow({ conversation, archived, selected, canOrganize, onOpen, onSetPinned, onArchive, onRestore, onCloseTemporary }: { conversation: NavigationConversation; archived: boolean; selected: boolean; canOrganize: boolean; onOpen(): void; onSetPinned(pinned: boolean): void; onArchive(): void; onRestore(): void; onCloseTemporary(): void }) {
  const canCloseTemporary = canOrganize && (
    conversation.status === "idle"
    || conversation.status === "failed"
    || conversation.status === "completed"
  )
  const buttonStyle = { cursor: canOrganize ? "pointer" : "default", padding: 4, backgroundColor: palette.panelRaised }
  return <div style={{ display: "flex", alignItems: "center", gap: 6, padding: 5, borderRadius: 7, backgroundColor: selected ? palette.panelRaised : palette.panel }}>
    <div testId={`conversation-entry-${conversation.sessionId}`} tabIndex={0} accessibilityRole="button" accessibilityName={`Open conversation ${conversation.title}`} accessibilitySelected={selected} onClick={onOpen} onKeyDown={(event) => { if (activates(event)) onOpen() }} style={{ flexGrow: 1, padding: 4, cursor: "pointer" }}><text style={{ color: palette.text, fontSize: 13 }}>{conversation.pinned ? "★ " : ""}{conversation.title}</text></div>
    {archived
      ? <div testId={`restore-conversation-${conversation.sessionId}`} tabIndex={canOrganize ? 0 : -1} accessibilityRole="button" accessibilityName={`Restore conversation ${conversation.title}`} accessibilityDisabled={!canOrganize} onClick={() => { if (canOrganize) onRestore() }} onKeyDown={(event) => { if (activates(event) && canOrganize) onRestore() }} style={buttonStyle}><text>Restore</text></div>
      : <><div testId={`pin-conversation-${conversation.sessionId}`} tabIndex={canOrganize ? 0 : -1} accessibilityRole="button" accessibilityName={`${conversation.pinned ? "Unpin" : "Pin"} conversation ${conversation.title}`} accessibilityDisabled={!canOrganize} onClick={() => { if (canOrganize) onSetPinned(!conversation.pinned) }} onKeyDown={(event) => { if (activates(event) && canOrganize) onSetPinned(!conversation.pinned) }} style={buttonStyle}><text>{conversation.pinned ? "Unpin" : "Pin"}</text></div>{conversation.placement.kind === "temporary" ? <div testId={`close-temporary-conversation-${conversation.sessionId}`} tabIndex={canCloseTemporary ? 0 : -1} accessibilityRole="button" accessibilityName={`Close temporary conversation ${conversation.title}`} accessibilityDisabled={!canCloseTemporary} onClick={() => { if (canCloseTemporary) onCloseTemporary() }} onKeyDown={(event) => { if (activates(event) && canCloseTemporary) onCloseTemporary() }} style={{ ...buttonStyle, cursor: canCloseTemporary ? "pointer" : "default" }}><text>Close</text></div> : <div testId={`archive-conversation-${conversation.sessionId}`} tabIndex={canOrganize ? 0 : -1} accessibilityRole="button" accessibilityName={`Archive conversation ${conversation.title}`} accessibilityDisabled={!canOrganize} onClick={() => { if (canOrganize) onArchive() }} onKeyDown={(event) => { if (activates(event) && canOrganize) onArchive() }} style={buttonStyle}><text>Archive</text></div>}</>}
  </div>
}
