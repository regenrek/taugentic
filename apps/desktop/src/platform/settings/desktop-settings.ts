import type { DockLayout } from "@gpuix/react"
import type { WorkspaceId } from "@taugentic/desktop-protocol"

export type DesktopTheme = "dark" | "light"

export type DesktopPresentation = {
  theme: DesktopTheme
  layout: DockLayout
}

type SettingsSnapshot = {
  presentations: Record<WorkspaceId, DesktopPresentation>
}

/**
 * The one current-shape presentation settings owner. Its callers must supply
 * a real WorkspaceId; therefore it cannot create a global or sentinel layout.
 */
export class DesktopSettings {
  #snapshot: SettingsSnapshot = { presentations: {} }
  #listeners = new Set<() => void>()

  presentation(workspaceId: WorkspaceId): DesktopPresentation | undefined {
    return this.#snapshot.presentations[workspaceId]
  }

  savePresentation(workspaceId: WorkspaceId, presentation: DesktopPresentation): void {
    this.#snapshot = {
      presentations: { ...this.#snapshot.presentations, [workspaceId]: presentation },
    }
    for (const listener of this.#listeners) listener()
  }

  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener)
    return () => this.#listeners.delete(listener)
  }
}

export const desktopSettings = new DesktopSettings()
