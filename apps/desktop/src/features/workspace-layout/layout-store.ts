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
      panels: ["conversation", "source", "diff"],
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

export function saveDesktopTheme(theme: DesktopTheme): void {
  desktopSettings.saveAppearance({ theme })
}

/** The persisted dock tree is the single owner of panel visibility. */
export function isDockPanelVisible(layout: DockLayout, panelId: string): boolean {
  if (layout.zoomed) return layout.zoomed === panelId
  if (layout.kind === "tabs") return layout.active === panelId
  return isDockPanelVisible(layout.first, panelId) || isDockPanelVisible(layout.second, panelId)
}
