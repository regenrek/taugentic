import type { NavigationConversation, NavigationSnapshot, ProjectId, SessionId } from "@taugentic/desktop-protocol"

import { palette } from "../../app/theme.js"

export type SidebarView = "spaces" | "projects" | "agents"

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
  if (action.type === "selectConversation") return { ...state, selectedConversationId: action.sessionId }
  if (action.type === "selectProject" || action.type === "openProject") return state
  if (action.type === "setFilter") return { ...state, filter: action.filter }
  const expanded = state.expandedSpaceIds.includes(action.spaceId)
    ? state.expandedSpaceIds.filter((id) => id !== action.spaceId)
    : [...state.expandedSpaceIds, action.spaceId]
  return { ...state, expandedSpaceIds: expanded }
}

function includesFilter(value: string, filter: string): boolean {
  return value.toLocaleLowerCase().includes(filter.trim().toLocaleLowerCase())
}

export function projectConversations(snapshot: NavigationSnapshot, projectId?: ProjectId): NavigationConversation[] {
  if (!projectId) return []
  return (snapshot.conversations ?? []).filter((conversation) => (
    !conversation.archived
    && conversation.placement.kind === "project"
    && conversation.placement.projectId === projectId
  ))
}

type SidebarProps = {
  snapshot: NavigationSnapshot
  state: SidebarState
  selectedProjectId?: ProjectId
  conversationTitle: string
  canCreateConversation: boolean
  dispatch(action: SidebarAction): void
  onConversationTitleChange(value: string): void
  onCreateConversation(): void
}

export function Sidebar({ snapshot, state, selectedProjectId, conversationTitle, canCreateConversation, dispatch, onConversationTitleChange, onCreateConversation }: SidebarProps) {
  const conversations = projectConversations(snapshot, selectedProjectId).filter((conversation) => includesFilter(conversation.title, state.filter))
  const projects = (snapshot.projects ?? []).filter((project) => includesFilter(project.title, state.filter))
  const spaces = (snapshot.spaces ?? []).filter((space) => includesFilter(space.title, state.filter))

  return <div testId="workspace-sidebar" style={{ display: "flex", flexDirection: "column", padding: 14, gap: 8, backgroundColor: palette.panel, minWidth: 220 }}>
    <text style={{ color: palette.textFaint, fontSize: 10, fontWeight: 700 }}>WORKSPACE</text>
    <input testId="sidebar-filter" value={state.filter} placeholder="Filter conversations" onChange={(event) => dispatch({ type: "setFilter", filter: event.value ?? "" })} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
    <div style={{ display: "flex", gap: 6 }}>
      {(["spaces", "projects", "agents"] as const).map((view) => <div testId={`sidebar-view-${view}`} tabIndex={0} onClick={() => dispatch({ type: "selectView", view })} style={{ cursor: "pointer", padding: 6, backgroundColor: state.view === view ? palette.panelRaised : palette.panel }}><text style={{ color: palette.textMuted, fontSize: 11 }}>{view}</text></div>)}
    </div>
    {state.view === "spaces" && spaces.map((space) => <div>
      <div testId={`space-${space.id}`} tabIndex={0} onClick={() => dispatch({ type: "toggleSpace", spaceId: space.id })} style={{ cursor: "pointer", padding: 7 }}><text>{state.expandedSpaceIds.includes(space.id) ? "⌄ " : "› "}{space.title}</text></div>
      {state.expandedSpaceIds.includes(space.id) && projects.filter((project) => project.spaceId === space.id).map((project) => <div testId={`project-${project.id}`} tabIndex={0} onClick={() => dispatch({ type: "selectProject", projectId: project.id })} onKeyDown={(event) => { if (event.key === "enter") dispatch({ type: "selectProject", projectId: project.id }) }} style={{ cursor: "pointer", padding: 7, marginLeft: 16, backgroundColor: selectedProjectId === project.id ? palette.panelRaised : palette.panel }}><text style={{ color: palette.textMuted, fontSize: 12 }}>{project.title}</text></div>)}
    </div>)}
    {state.view === "projects" && projects.map((project) => <div testId={`project-${project.id}`} tabIndex={0} onClick={() => dispatch({ type: "selectProject", projectId: project.id })} onKeyDown={(event) => { if (event.key === "enter") dispatch({ type: "selectProject", projectId: project.id }) }} style={{ cursor: "pointer", padding: 7, backgroundColor: selectedProjectId === project.id ? palette.panelRaised : palette.panel }}><text style={{ color: palette.textMuted, fontSize: 12 }}>{project.title}</text></div>)}
    {state.view === "projects" && <div testId="sidebar-open-project" tabIndex={0} onClick={() => dispatch({ type: "openProject" })} onKeyDown={(event) => { if (event.key === "enter") dispatch({ type: "openProject" }) }} style={{ cursor: "pointer", padding: 8, backgroundColor: palette.panelRaised }}><text>Open project</text></div>}
    {state.view === "agents" && (snapshot.agents ?? []).map((agent) => <text style={{ color: palette.textMuted, fontSize: 12, padding: 7 }}>{agent.title}</text>)}
    <text style={{ color: palette.textFaint, fontSize: 10, fontWeight: 700, marginTop: 8 }}>CONVERSATIONS</text>
    {selectedProjectId && <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <input testId="new-conversation-title" value={conversationTitle} placeholder="Conversation title" onChange={(event) => onConversationTitleChange(event.value ?? "")} onKeyDown={(event) => { if (event.key === "enter") onCreateConversation() }} style={{ height: 32, paddingLeft: 8, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
      <div testId="create-conversation" tabIndex={0} onClick={onCreateConversation} onKeyDown={(event) => { if (event.key === "enter") onCreateConversation() }} style={{ cursor: "pointer", padding: 8, backgroundColor: canCreateConversation ? palette.accentDim : palette.panelRaised }}><text>New conversation</text></div>
    </div>}
    {conversations.map((conversation) => <div testId="conversation-entry" tabIndex={0} onClick={() => dispatch({ type: "selectConversation", sessionId: conversation.sessionId })} onKeyDown={(event) => { if (event.key === "enter") dispatch({ type: "selectConversation", sessionId: conversation.sessionId }) }} style={{ padding: 9, borderRadius: 7, cursor: "pointer", backgroundColor: state.selectedConversationId === conversation.sessionId ? palette.panelRaised : palette.panel }}><text style={{ color: palette.text, fontSize: 13 }}>{conversation.title}</text></div>)}
    {!conversations.length && <text style={{ color: palette.textMuted, fontSize: 12 }}>No conversations available.</text>}
  </div>
}
