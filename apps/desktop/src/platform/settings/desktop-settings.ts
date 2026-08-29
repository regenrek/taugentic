import type { DockLayout } from "@regenrek/gpuix-react"
import type { WorkspaceId } from "@taugentic/desktop-protocol"

import { commandById, commandRegistry, normalizeShortcut, type CommandId } from "../../features/commands/registry.js"

export type DesktopTheme = "dark" | "light"
export type DesktopContrast = "standard" | "high"
export type DesktopFontScale = "standard" | "large"

export type DesktopAppearance = {
  theme: DesktopTheme
  contrast: DesktopContrast
  fontScale: DesktopFontScale
  reducedMotion: boolean
}

export type DesktopPresentation = { theme: DesktopTheme; layout: DockLayout }

export type DesktopSettingsDocument = {
  appearance: DesktopAppearance
  layouts: Record<WorkspaceId, DockLayout>
  shortcuts: Partial<Record<CommandId, string>>
}

export type DesktopSettingsPersistence = {
  read(): Promise<string | null>
  write(documentJson: string): Promise<void>
}

export const defaultDesktopAppearance: DesktopAppearance = {
  theme: "dark",
  contrast: "standard",
  fontScale: "standard",
  reducedMotion: false,
}

function emptyDocument(): DesktopSettingsDocument {
  return { appearance: { ...defaultDesktopAppearance }, layouts: {}, shortcuts: {} }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function hasOnlyKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(value).every((key) => keys.includes(key))
}

function isDockLayout(value: unknown): value is DockLayout {
  if (!isRecord(value) || typeof value.id !== "string") return false
  if (value.kind === "tabs") {
    return Array.isArray(value.panels)
      && value.panels.every((panel) => typeof panel === "string")
      && typeof value.active === "string"
      && hasOnlyKeys(value, ["kind", "id", "panels", "active", "zoomed"])
      && (value.zoomed === undefined || typeof value.zoomed === "string")
  }
  return value.kind === "split"
    && (value.direction === "horizontal" || value.direction === "vertical")
    && typeof value.ratio === "number"
    && Number.isFinite(value.ratio)
    && value.ratio > 0
    && value.ratio < 1
    && isDockLayout(value.first)
    && isDockLayout(value.second)
    && hasOnlyKeys(value, ["kind", "id", "direction", "ratio", "first", "second", "zoomed"])
    && (value.zoomed === undefined || typeof value.zoomed === "string")
}

function parseDocument(documentJson: string): DesktopSettingsDocument {
  let candidate: unknown
  try { candidate = JSON.parse(documentJson) } catch { throw new Error("Desktop settings data is malformed.") }
  if (!isRecord(candidate) || !hasOnlyKeys(candidate, ["appearance", "layouts", "shortcuts"])) throw new Error("Desktop settings data is malformed.")
  const { appearance, layouts, shortcuts } = candidate
  if (!isRecord(appearance) || !hasOnlyKeys(appearance, ["theme", "contrast", "fontScale", "reducedMotion"])
    || (appearance.theme !== "dark" && appearance.theme !== "light")
    || (appearance.contrast !== "standard" && appearance.contrast !== "high")
    || (appearance.fontScale !== "standard" && appearance.fontScale !== "large")
    || typeof appearance.reducedMotion !== "boolean"
    || !isRecord(layouts) || !isRecord(shortcuts)) throw new Error("Desktop settings data is malformed.")
  for (const [workspaceId, layout] of Object.entries(layouts)) {
    if (!workspaceId || !isDockLayout(layout)) throw new Error("Desktop settings data is malformed.")
  }
  for (const [commandId, shortcut] of Object.entries(shortcuts)) {
    if (!commandById(commandId as CommandId) || typeof shortcut !== "string" || normalizeShortcut(shortcut) !== shortcut) {
      throw new Error("Desktop settings data is malformed.")
    }
  }
  return {
    appearance: { theme: appearance.theme, contrast: appearance.contrast, fontScale: appearance.fontScale, reducedMotion: appearance.reducedMotion },
    layouts: layouts as Record<WorkspaceId, DockLayout>,
    shortcuts: shortcuts as Partial<Record<CommandId, string>>,
  }
}

/** Sole semantic owner for the device-local current-shape presentation document. */
export class DesktopSettings {
  #document: DesktopSettingsDocument = emptyDocument()
  #listeners = new Set<() => void>()
  #persistence?: DesktopSettingsPersistence
  #initialization?: Promise<void>
  #error?: string
  #loaded = false
  #revision = 0

  appearance(): DesktopAppearance { return this.#document.appearance }
  presentation(workspaceId: WorkspaceId): DesktopPresentation | undefined {
    const layout = this.#document.layouts[workspaceId]
    return layout ? { theme: this.#document.appearance.theme, layout } : undefined
  }
  shortcut(commandId: CommandId): string | undefined { return this.#document.shortcuts[commandId] }
  error(): string | undefined { return this.#error }
  loaded(): boolean { return this.#loaded }
  revision = (): number => this.#revision

  initialize(persistence: DesktopSettingsPersistence): Promise<void> {
    if (!this.#initialization) this.#initialization = this.#load(persistence)
    return this.#initialization
  }

  async #load(persistence: DesktopSettingsPersistence): Promise<void> {
    this.#persistence = persistence
    try {
      const documentJson = await persistence.read()
      if (documentJson !== null) this.#document = parseDocument(documentJson)
      this.#loaded = true
      this.#error = undefined
    } catch {
      this.#error = "Desktop settings could not be loaded. Fix or remove the local settings document before changing preferences."
    }
    this.#notify()
  }

  saveLayout(workspaceId: WorkspaceId, layout: DockLayout): void {
    this.#commit({ ...this.#document, layouts: { ...this.#document.layouts, [workspaceId]: layout } })
  }

  deleteLayout(workspaceId: WorkspaceId): void {
    if (!(workspaceId in this.#document.layouts)) return
    const { [workspaceId]: _, ...layouts } = this.#document.layouts
    this.#commit({ ...this.#document, layouts })
  }

  saveAppearance(patch: Partial<DesktopAppearance>): void {
    this.#commit({ ...this.#document, appearance: { ...this.#document.appearance, ...patch } })
  }

  saveShortcut(commandId: CommandId, shortcut: string): "saved" | "conflict" {
    const normalized = normalizeShortcut(shortcut)
    const shortcuts = normalized
      ? { ...this.#document.shortcuts, [commandId]: normalized }
      : Object.fromEntries(Object.entries(this.#document.shortcuts).filter(([id]) => id !== commandId)) as Partial<Record<CommandId, string>>
    const effectiveShortcut = (id: CommandId) => shortcuts[id] ?? commandById(id)?.defaultShortcut
    const effective = effectiveShortcut(commandId)
    if (effective && commandRegistry.some((command) => command.id !== commandId && normalizeShortcut(effectiveShortcut(command.id) ?? "") === normalizeShortcut(effective))) return "conflict"
    this.#commit({ ...this.#document, shortcuts })
    return "saved"
  }

  subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener)
    return () => this.#listeners.delete(listener)
  }

  #commit(document: DesktopSettingsDocument): void {
    if (this.#error) return
    this.#document = document
    this.#notify()
    if (!this.#persistence) return
    void this.#persistence.write(JSON.stringify(document)).catch(() => {
      this.#error = "Desktop settings could not be saved."
      this.#notify()
    })
  }

  #notify(): void {
    this.#revision += 1
    for (const listener of this.#listeners) listener()
  }
}

export const desktopSettings = new DesktopSettings()
