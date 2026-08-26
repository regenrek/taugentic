import type { DockLayout } from "@gpuix/react"
import type { WorkspaceId } from "@taugentic/desktop-protocol"

import { desktopSettings, type DesktopPresentation, type DesktopTheme } from "../../platform/settings/desktop-settings.js"

export const defaultWorkspaceLayout: DockLayout = {
  kind: "tabs",
  id: "workspace-root",
  panels: ["conversation", "activity"],
  active: "conversation",
}

export function workspacePresentation(workspaceId: WorkspaceId): DesktopPresentation {
  const persisted = desktopSettings.presentation(workspaceId)
  if (persisted) return persisted
  const presentation = { theme: "dark", layout: defaultWorkspaceLayout } satisfies DesktopPresentation
  desktopSettings.savePresentation(workspaceId, presentation)
  return presentation
}

export function saveWorkspaceLayout(workspaceId: WorkspaceId, layout: DockLayout): void {
  const current = workspacePresentation(workspaceId)
  desktopSettings.savePresentation(workspaceId, { ...current, layout })
}

export function saveWorkspaceTheme(workspaceId: WorkspaceId, theme: DesktopTheme): void {
  const current = workspacePresentation(workspaceId)
  desktopSettings.savePresentation(workspaceId, { ...current, theme })
}
