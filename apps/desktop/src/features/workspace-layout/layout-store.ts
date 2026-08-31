import type { DockLayout } from "@regenrek/gpuix-react"
import type { WorkspaceId } from "@taugentic/desktop-protocol"

import { desktopSettings, type DesktopPresentation, type DesktopTheme } from "../../platform/settings/desktop-settings.js"

export const defaultWorkspaceLayout: DockLayout = {
  kind: "split",
  id: "workspace-root",
  direction: "horizontal",
  ratio: 0.22,
  first: {
    kind: "split",
    id: "workspace-navigation",
    direction: "vertical",
    ratio: 0.64,
    first: {
      kind: "tabs",
      id: "workspace-files",
      panels: ["files"],
      active: "files",
    },
    second: {
      kind: "tabs",
      id: "workspace-artifacts",
      panels: ["artifacts"],
      active: "artifacts",
    },
  },
  second: {
    kind: "split",
    id: "workspace-content",
    direction: "horizontal",
    ratio: 0.68,
    first: {
      kind: "tabs",
      id: "workspace-primary",
      panels: ["conversation", "source", "diff", "browser"],
      active: "conversation",
    },
    second: {
      kind: "tabs",
      id: "workspace-inspection",
      panels: ["activity", "thread-workspace", "git", "pull-requests", "terminal", "image", "pdf"],
      active: "git",
    },
  },
}

/** An unconfigured workspace reads the desktop-owned appearance without persisting. */
export function workspacePresentation(workspaceId?: WorkspaceId): DesktopPresentation {
  const configured = workspaceId === undefined ? undefined : desktopSettings.presentation(workspaceId)
  return configured ?? { theme: desktopSettings.appearance().theme, layout: defaultWorkspaceLayout }
}

export function saveWorkspaceLayout(workspaceId: WorkspaceId, layout: DockLayout): void {
  desktopSettings.saveLayout(workspaceId, layout)
}

/** Opens one panel through the persisted dock tree without creating parallel visibility state. */
export function openDockPanel(layout: DockLayout, panelId: string, targetTabsId: string): DockLayout {
  let found = false
  const activate = (node: DockLayout): DockLayout => {
    if (node.kind === "tabs") {
      if (!node.panels.includes(panelId)) return node.zoomed === undefined ? node : { ...node, zoomed: undefined }
      found = true
      return { ...node, active: panelId, zoomed: undefined }
    }
    return { ...node, zoomed: undefined, first: activate(node.first), second: activate(node.second) }
  }
  const activated = activate(layout)
  if (found) return activated

  let inserted = false
  const insert = (node: DockLayout): DockLayout => {
    if (node.kind === "tabs") {
      if (node.id !== targetTabsId) return node
      inserted = true
      return { ...node, panels: [...node.panels, panelId], active: panelId }
    }
    return { ...node, first: insert(node.first), second: insert(node.second) }
  }
  const opened = insert(activated)
  if (!inserted) throw new Error(`Dock target ${targetTabsId} is missing.`)
  return opened
}

/** Removes one panel from the persisted dock tree and collapses empty branches. */
export function closeDockPanel(layout: DockLayout, panelId: string): DockLayout {
  const remove = (node: DockLayout): DockLayout | undefined => {
    if (node.kind === "tabs") {
      if (!node.panels.includes(panelId)) return node
      const panels = node.panels.filter((candidate) => candidate !== panelId)
      if (panels.length === 0) return undefined
      return {
        ...node,
        panels,
        active: node.active === panelId ? panels[0] : node.active,
        zoomed: node.zoomed === panelId ? undefined : node.zoomed,
      }
    }
    const first = remove(node.first)
    const second = remove(node.second)
    if (!first) return second
    if (!second) return first
    return {
      ...node,
      zoomed: node.zoomed === panelId ? undefined : node.zoomed,
      first,
      second,
    }
  }
  return remove(layout) ?? layout
}

export function resetWorkspaceLayout(workspaceId: WorkspaceId): void {
  desktopSettings.deleteLayout(workspaceId)
}

export function saveDesktopTheme(theme: DesktopTheme): void {
  desktopSettings.saveAppearance({ theme })
}

/** The persisted dock tree is also the single owner of panel mount lifetime. */
export function hasDockPanel(layout: DockLayout, panelId: string): boolean {
  if (layout.kind === "tabs") return layout.panels.includes(panelId)
  return hasDockPanel(layout.first, panelId) || hasDockPanel(layout.second, panelId)
}

/** The persisted dock tree is the single owner of panel visibility. */
export function isDockPanelVisible(layout: DockLayout, panelId: string): boolean {
  if (layout.zoomed) return layout.zoomed === panelId
  if (layout.kind === "tabs") return layout.active === panelId
  return isDockPanelVisible(layout.first, panelId) || isDockPanelVisible(layout.second, panelId)
}
