export type DesktopCommand = {
  id: "focus-conversation" | "focus-activity" | "toggle-theme"
  title: string
  panelId?: "conversation" | "activity"
}

/** One declarative command metadata registry; dispatch remains shell-owned. */
export const commandRegistry: readonly DesktopCommand[] = [
  { id: "focus-conversation", title: "Focus conversation", panelId: "conversation" },
  { id: "focus-activity", title: "Focus activity", panelId: "activity" },
  { id: "toggle-theme", title: "Toggle theme" },
]
