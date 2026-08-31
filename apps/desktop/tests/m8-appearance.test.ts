import { afterEach, describe, expect, it } from "bun:test"
import { readFile } from "node:fs/promises"
import { resolve } from "node:path"

import { applyDesktopAppearance, fontSize, metrics } from "../src/app/theme.js"

const desktopRoot = resolve(import.meta.dirname, "..")

afterEach(() => {
  applyDesktopAppearance({
    theme: "dark",
    contrast: "standard",
    fontScale: "standard",
    reducedMotion: false,
  })
})

describe("desktop appearance font scale", () => {
  it("keeps semantic sizes at identity for standard appearance", () => {
    applyDesktopAppearance({ theme: "dark", contrast: "standard", fontScale: "standard", reducedMotion: false })

    expect(metrics.fontScale).toBe(1)
    expect(fontSize(12)).toBe(12)
  })

  it("applies the existing large scale through the canonical theme helper", () => {
    applyDesktopAppearance({ theme: "dark", contrast: "standard", fontScale: "large", reducedMotion: false })

    expect(metrics.fontScale).toBe(1.18)
    expect(fontSize(12)).toBe(14.16)
  })

  it("leaves no numeric font-size owner in desktop presentation source", async () => {
    const sourceRoot = resolve(desktopRoot, "src")
    const presentationFiles = [
      "app/App.tsx", "app/product-state.tsx", "features/approvals/approvals-inbox-panel.tsx",
      "features/artifacts/artifacts-panel.tsx", "features/auth-profiles/auth-profiles.tsx",
      "features/code-host/pull-requests-panel.tsx", "features/commands/command-surface.tsx",
      "features/conversation-branches/branch-graph.tsx", "features/diagnostics/diagnostics-panel.tsx",
      "features/files/file-panels.tsx", "features/git/git-panel.tsx", "features/plugins/plugins-panel.tsx",
      "features/run-activity/run-activity-panel.tsx", "features/scheduled-work/scheduled-work-panel.tsx",
      "features/sidebar/sidebar.tsx", "features/terminal/terminal-panel.tsx",
      "features/thread-workspace/thread-workspace-panel.tsx", "features/voice/voice-panel.tsx",
      "features/work-items/work-inbox-panel.tsx", "features/workspace-layout/panels.tsx",
      "ui/copy-text-button.tsx",
    ]

    for (const relativePath of presentationFiles) {
      const source = await readFile(resolve(sourceRoot, relativePath), "utf8")
      expect(source).not.toMatch(/fontSize:\s*\d/)
    }
  })
})
