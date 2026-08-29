import type { DesktopAppearance } from "../platform/settings/desktop-settings.js"

/** Mutable render tokens derived only from the desktop presentation owner. */
export const palette = {
  canvas: "#0A0C0F", scrim: "rgba(10, 12, 15, 0.78)", panel: "#101318", panelRaised: "#151922",
  border: "#252B36", borderStrong: "#343C49", text: "#F2F4F7", textMuted: "#929AA8",
  textFaint: "#626B79", accent: "#A6FFCB", accentDim: "#143628", warning: "#F5C76B",
}

export const metrics = { titlebarHeight: 54, sidebarWidth: 276, panelRadius: 10, fontScale: 1, reducedMotion: false }

export function applyDesktopAppearance(appearance: DesktopAppearance): void {
  const light = appearance.theme === "light"
  const high = appearance.contrast === "high"
  Object.assign(palette, light
    ? { canvas: "#F6F7F9", scrim: "rgba(20, 25, 35, 0.38)", panel: "#FFFFFF", panelRaised: "#EDF0F5", border: high ? "#19202B" : "#B6BECA", borderStrong: "#46505E", text: "#111827", textMuted: "#3B4656", textFaint: "#647084", accent: "#0D6B42", accentDim: "#C8F0D9", warning: "#8A4B00" }
    : { canvas: "#0A0C0F", scrim: "rgba(10, 12, 15, 0.78)", panel: "#101318", panelRaised: "#151922", border: high ? "#AEB8C8" : "#252B36", borderStrong: "#596575", text: "#F2F4F7", textMuted: high ? "#D1D7E0" : "#929AA8", textFaint: "#788394", accent: "#A6FFCB", accentDim: "#143628", warning: "#F5C76B" })
  metrics.fontScale = appearance.fontScale === "large" ? 1.18 : 1
  metrics.reducedMotion = appearance.reducedMotion
}
