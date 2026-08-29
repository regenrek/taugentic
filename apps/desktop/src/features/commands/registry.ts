import type { DesktopSettings } from "../../platform/settings/desktop-settings.js"

export type FocusablePanelId = "conversation" | "activity" | "thread-workspace" | "git" | "pull-requests" | "terminal" | "image"
export const commandIds = ["open-settings", "focus-conversation", "focus-activity", "focus-thread-workspace", "focus-git", "focus-pull-requests", "focus-terminal", "toggle-theme", "start-run", "cancel-run"] as const
export type CommandId = typeof commandIds[number]
export type CommandContext = { canStart: boolean; canCancel: boolean }
export type DesktopCommand = { id: CommandId; title: string; defaultShortcut?: string; panelId?: FocusablePanelId; enabled(context: CommandContext): boolean }
export type CommandActions = { openSettings(): void; focusPanel(panelId: FocusablePanelId): void; toggleTheme(): void; startRun(): void; cancelRun(): void }
export type CommandDispatcher = { dispatch(id: CommandId): boolean; shortcutFor(id: CommandId): string | undefined; commandForShortcut(shortcut: string): CommandId | undefined; enabled(id: CommandId): boolean }

/** The sole command contract: ids, labels, enablement, and default shortcuts. */
export const commandRegistry: readonly DesktopCommand[] = [
  { id: "open-settings", title: "Open settings", defaultShortcut: "mod+,", enabled: () => true },
  { id: "focus-conversation", title: "Focus conversation", panelId: "conversation", defaultShortcut: "mod+1", enabled: () => true },
  { id: "focus-activity", title: "Focus activity", panelId: "activity", defaultShortcut: "mod+2", enabled: () => true },
  { id: "focus-thread-workspace", title: "Focus thread workspace", panelId: "thread-workspace", defaultShortcut: "mod+6", enabled: () => true },
  { id: "focus-git", title: "Focus Git", panelId: "git", defaultShortcut: "mod+3", enabled: () => true },
  { id: "focus-pull-requests", title: "Focus pull requests", panelId: "pull-requests", defaultShortcut: "mod+4", enabled: () => true },
  { id: "focus-terminal", title: "Focus terminal", panelId: "terminal", defaultShortcut: "mod+5", enabled: () => true },
  { id: "toggle-theme", title: "Toggle theme", defaultShortcut: "mod+shift+t", enabled: () => true },
  { id: "start-run", title: "Start run", defaultShortcut: "mod+enter", enabled: (context) => context.canStart },
  { id: "cancel-run", title: "Cancel run", defaultShortcut: "mod+shift+enter", enabled: (context) => context.canCancel },
]

export function commandById(id: CommandId): DesktopCommand | undefined {
  return commandRegistry.find((command) => command.id === id)
}

export function normalizeShortcut(shortcut: string): string { return shortcut.trim().toLowerCase().replaceAll("meta", "mod").replaceAll("control", "mod") }
export function eventShortcut(event: { key?: string; modifiers?: { cmd: boolean; ctrl: boolean; shift: boolean } }): string { return [event.modifiers?.cmd || event.modifiers?.ctrl ? "mod" : "", event.modifiers?.shift ? "shift" : "", event.key?.toLowerCase() ?? ""].filter(Boolean).join("+") }

/** The sole dispatcher. Palette, menus, composer and shortcuts invoke ids here. */
export function createCommandDispatcher(settings: DesktopSettings, context: () => CommandContext, actions: CommandActions): CommandDispatcher {
  const shortcutFor = (id: CommandId) => settings.shortcut(id) ?? commandById(id)?.defaultShortcut
  return {
    dispatch(id) { const current = commandById(id); if (!current || !current.enabled(context())) return false; if (current.panelId) actions.focusPanel(current.panelId); else if (id === "open-settings") actions.openSettings(); else if (id === "toggle-theme") actions.toggleTheme(); else if (id === "start-run") actions.startRun(); else if (id === "cancel-run") actions.cancelRun(); return true },
    shortcutFor,
    commandForShortcut(shortcut) {
      const normalized = normalizeShortcut(shortcut)
      const matches = commandRegistry.filter((candidate) => shortcutFor(candidate.id) && normalizeShortcut(shortcutFor(candidate.id)!) === normalized)
      return matches.length === 1 ? matches[0]!.id : undefined
    },
    enabled(id) { return commandById(id)?.enabled(context()) ?? false },
  }
}
