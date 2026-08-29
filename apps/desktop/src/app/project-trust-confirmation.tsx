import { palette } from "./theme.js"

function activates(event: { key?: string }): boolean { return event.key === "enter" || event.key === "space" }

export function ProjectTrustConfirmation({ onDecision }: { onDecision: (acknowledged: boolean) => void }) {
  return <div
    testId="project-trust-confirmation"
    accessibilityRole="alertdialog"
    accessibilityName="Project trust confirmation"
    style={{ position: "absolute", left: 0, right: 0, top: 0, bottom: 0, display: "flex", alignItems: "center", justifyContent: "center", backgroundColor: palette.scrim }}
    onKeyDown={(event) => { if (event.key === "escape") onDecision(false) }}
  ><div style={{ padding: 18, gap: 12, backgroundColor: palette.panelRaised, borderWidth: 1, borderColor: palette.border }}><text>Trust this folder and allow Taugentic to work with its files?</text><div style={{ display: "flex", gap: 8 }}><div
    testId="decline-project-trust"
    autoFocus
    tabIndex={0}
    accessibilityRole="button"
    accessibilityName="Cancel project trust"
    onClick={() => onDecision(false)}
    onKeyDown={(event) => { if (activates(event)) onDecision(false) }}
    style={{ cursor: "pointer", padding: 8, backgroundColor: palette.panel }}
  ><text>Cancel</text></div><div
    testId="confirm-project-trust"
    tabIndex={0}
    accessibilityRole="button"
    accessibilityName="Trust and open project"
    onClick={() => onDecision(true)}
    onKeyDown={(event) => { if (activates(event)) onDecision(true) }}
    style={{ cursor: "pointer", padding: 8, backgroundColor: palette.accent }}
  ><text>Trust and open</text></div></div></div></div>
}
