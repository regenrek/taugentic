import { describe, expect, it } from "bun:test"
import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { useMemo, useState } from "react"

import { Workbench } from "../src/app/App.js"
import { CommandSurface } from "../src/features/commands/command-surface.js"
import { createCommandDispatcher, eventShortcut } from "../src/features/commands/registry.js"
import { ConversationPanel } from "../src/features/workspace-layout/panels.js"
import { workspacePresentation } from "../src/features/workspace-layout/layout-store.js"
import { desktopSettings, DesktopSettings } from "../src/platform/settings/desktop-settings.js"

describe("M2 command surface", () => {
  it("routes shortcuts and visible command ids through one dispatcher", () => {
    const calls: string[] = []
    const settings = new DesktopSettings()
    const dispatcher = createCommandDispatcher(settings, () => ({ canStart: true, canCancel: true }), {
      openSettings: () => calls.push("settings"),
      openProject: () => calls.push("project"),
      openDiagnostics: () => calls.push("diagnostics"),
      openPlugins: () => calls.push("plugins"),
      openBrowser: () => calls.push("browser"),
      focusPanel: (panel) => calls.push(`focus:${panel}`),
      toggleTheme: () => calls.push("theme"),
      startRun: () => calls.push("start"),
      cancelRun: () => calls.push("cancel"),
    })

    expect(dispatcher.commandForShortcut(eventShortcut({ key: "1", modifiers: { cmd: true, ctrl: false, shift: false } }))).toBe("focus-conversation")
    expect(dispatcher.dispatch("focus-conversation")).toBe(true)
    expect(dispatcher.dispatch("open-project")).toBe(true)
    expect(dispatcher.dispatch("open-diagnostics")).toBe(true)
    expect(dispatcher.dispatch("open-plugins")).toBe(true)
    expect(dispatcher.dispatch("open-browser")).toBe(true)
    expect(dispatcher.dispatch("start-run")).toBe(true)
    expect(dispatcher.dispatch("cancel-run")).toBe(true)
    expect(calls).toEqual(["focus:conversation", "project", "diagnostics", "plugins", "browser", "start", "cancel"])
  })

  it("keeps global commands operable while workspace-bound commands are disabled", () => {
    const calls: string[] = []
    const dispatcher = createCommandDispatcher(new DesktopSettings(), () => ({ canStart: false, canCancel: false, hasWorkspace: false }), {
      openSettings: () => calls.push("settings"), openProject: () => calls.push("project"), openDiagnostics: () => calls.push("diagnostics"), openPlugins: () => calls.push("plugins"), openBrowser: () => calls.push("browser"), focusPanel: () => calls.push("focus"), toggleTheme: () => calls.push("theme"), startRun: () => calls.push("start"), cancelRun: () => calls.push("cancel"),
    })

    expect(dispatcher.enabled("open-project")).toBe(true)
    expect(dispatcher.enabled("open-diagnostics")).toBe(true)
    expect(dispatcher.enabled("open-plugins")).toBe(true)
    expect(dispatcher.enabled("open-browser")).toBe(false)
    expect(dispatcher.enabled("focus-conversation")).toBe(false)
    expect(dispatcher.dispatch("open-project")).toBe(true)
    expect(dispatcher.dispatch("open-diagnostics")).toBe(true)
    expect(dispatcher.dispatch("open-plugins")).toBe(true)
    expect(dispatcher.dispatch("open-browser")).toBe(false)
    expect(calls).toEqual(["project", "diagnostics", "plugins"])
  })

  it("keeps shortcut overrides device-local presentation settings", () => {
    const settings = new DesktopSettings()
    settings.saveShortcut("focus-git", "mod+g")
    const dispatcher = createCommandDispatcher(settings, () => ({ canStart: false, canCancel: false }), { openSettings() {}, openProject() {}, openDiagnostics() {}, openPlugins() {}, openBrowser() {}, focusPanel() {}, toggleTheme() {}, startRun() {}, cancelRun() {} })

    expect(dispatcher.shortcutFor("focus-git")).toBe("mod+g")
    expect(dispatcher.commandForShortcut("mod+g")).toBe("focus-git")
    expect(dispatcher.dispatch("start-run")).toBe(false)
  })

  it("rejects normalized collisions across default and overridden effective shortcuts", () => {
    const settings = new DesktopSettings()

    expect(settings.saveShortcut("focus-git", "meta+2")).toBe("conflict")
    expect(settings.saveShortcut("focus-git", "mod+g")).toBe("saved")
    expect(settings.saveShortcut("focus-activity", "mod+g")).toBe("conflict")
    expect(settings.shortcut("focus-git")).toBe("mod+g")
  })

  it("routes a native Workbench shortcut through the product handler", () => {
    const calls: string[] = []
    const settings = new DesktopSettings()
    const dispatcher = createCommandDispatcher(settings, () => ({ canStart: false, canCancel: false }), {
      openSettings: () => calls.push("settings"), openProject: () => calls.push("project"), openDiagnostics: () => calls.push("diagnostics"), openPlugins: () => calls.push("plugins"),
      openBrowser: () => calls.push("browser"),
      focusPanel: (panel) => calls.push(`focus:${panel}`), toggleTheme: () => calls.push("theme"), startRun: () => calls.push("start"), cancelRun: () => calls.push("cancel"),
    })
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(<Workbench workspaceId="workspace-command-workbench" presentation={workspacePresentation("workspace-command-workbench")} panels={[{ id: "conversation", label: "Conversation", content: <div /> }]} commands={dispatcher} />)
      const workspace = renderer.findByTestId("workspace-dock")!
      renderer.nativeSimulateKeystrokes(workspace.id, "cmd-1")
      expect(calls).toEqual(["focus:conversation"])
    } finally { unmount() }
  })

  it("uses native palette, focused visible menu activation, disabled accessibility, and trigger restoration", () => {
    const calls: string[] = []
    const settings = new DesktopSettings()
    const dispatcher = createCommandDispatcher(settings, () => ({ canStart: false, canCancel: false }), {
      openSettings: () => calls.push("settings"), openProject: () => calls.push("project"), openDiagnostics: () => calls.push("diagnostics"), openPlugins: () => calls.push("plugins"),
      openBrowser: () => calls.push("browser"),
      focusPanel: (panel) => calls.push(`focus:${panel}`), toggleTheme: () => calls.push("theme"), startRun: () => calls.push("start"), cancelRun: () => calls.push("cancel"),
    })
    const { render, renderer, unmount } = createTestRoot()
    const click = (testId: string) => { const element = renderer.findByTestId(testId)!; const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id)!; renderer.nativeSimulateClick(x + width / 2, y + height / 2) }
    try {
      render(<CommandSurface dispatcher={dispatcher} settings={settings} settingsOpen={false} onSettingsOpenChange={() => {}} />)
      expect(renderer.getAutomationTree()).toContain("Open command palette")
      click("command-palette-toggle")
      const initialPaletteInput = renderer.findByTestId("command-palette-input")!
      expect(initialPaletteInput).toBeDefined()
      renderer.nativeSimulateKeystrokes(initialPaletteInput.id, "enter")
      expect(calls).toEqual(["settings"])
      expect(renderer.findByTestId("command-palette-input")).toBeUndefined()
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("command-palette-toggle")!.id, "enter")
      expect(renderer.findByTestId("command-palette-input")).toBeDefined()
      const paletteInput = renderer.findByTestId("command-palette-input")!
      renderer.nativeSimulateInput(paletteInput.id, "focus")
      renderer.simulateKeystrokes("down enter")
      expect(calls).toEqual(["settings", "focus:activity"])
      const dispatchPaletteQuery = (query: string, title: string) => {
        renderer.nativeSimulateKeystrokes(renderer.findByTestId("command-palette-toggle")!.id, "enter")
        const input = renderer.findByTestId("command-palette-input")!
        renderer.nativeSimulateInput(input.id, query)
        expect(renderer.getAutomationTree()).toContain(title)
        renderer.nativeSimulateKeystrokes(input.id, "enter")
      }
      dispatchPaletteQuery("project", "Open project")
      dispatchPaletteQuery("diagnostics", "Open diagnostics")
      dispatchPaletteQuery("plugins", "Open plugins")
      expect(calls).toEqual(["settings", "focus:activity", "project", "diagnostics", "plugins"])
      const menuTrigger = renderer.findByTestId("command-menu")!
      renderer.nativeSimulateKeystrokes(menuTrigger.id, "enter")
      expect(renderer.findByTestId("visible-command-menu")).toBeDefined()
      const visibleCommand = renderer.findByTestId("visible-command-focus-activity")!
      renderer.nativeSimulateKeystrokes(visibleCommand.id, "enter")
      expect(calls).toEqual(["settings", "focus:activity", "project", "diagnostics", "plugins", "focus:activity"])
      expect(renderer.findByTestId("visible-command-menu")).toBeUndefined()
      renderer.simulateKeystrokes("enter")
      expect(renderer.findByTestId("visible-command-menu")).toBeDefined()
      const disabledCommand = renderer.findByTestId("visible-command-start-run")!
      expect(renderer.getAutomationTree()).toContain("Start run")
      expect(renderer.getAutomationTree()).toContain("disabled")
      click("visible-command-start-run")
      renderer.nativeSimulateKeystrokes(disabledCommand.id, "enter")
      expect(calls).toEqual(["settings", "focus:activity", "project", "diagnostics", "plugins", "focus:activity"])
      render(<CommandSurface dispatcher={dispatcher} settings={settings} settingsOpen onSettingsOpenChange={() => {}} />)
      const shortcut = renderer.findByTestId("shortcut-focus-git")!
      renderer.nativeSimulateKeystrokes(shortcut.id, "cmd-a")
      renderer.nativeSimulateInput(shortcut.id, "mod+2")
      expect(renderer.findByTestId("shortcut-conflict")).toBeDefined()
      expect(dispatcher.dispatch("start-run")).toBe(false)
    } finally { unmount() }
  })

  it("derives every mutable settings control from the settings error while Close remains available", async () => {
    const settings = new DesktopSettings()
    await settings.initialize({ read: async () => "{", write: async () => {} })
    const dispatcher = createCommandDispatcher(settings, () => ({ canStart: false, canCancel: false }), { openSettings() {}, openProject() {}, openDiagnostics() {}, openPlugins() {}, openBrowser() {}, focusPanel() {}, toggleTheme() {}, startRun() {}, cancelRun() {} })
    const { render, renderer, unmount } = createTestRoot()
    const click = (testId: string) => { const element = renderer.findByTestId(testId)!; const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id)!; renderer.nativeSimulateClick(x + width / 2, y + height / 2) }
    let closed = 0
    try {
      render(<CommandSurface dispatcher={dispatcher} settings={settings} settingsOpen onSettingsOpenChange={(open) => { if (!open) closed += 1 }} />)
      const theme = renderer.findByTestId("setting-theme-light")!
      const motion = renderer.findByTestId("reduced-motion")!
      const shortcut = renderer.findByTestId("shortcut-focus-git")!
      expect(renderer.getAutomationTree()).toContain("Desktop settings could not be loaded")
      expect(renderer.getAutomationTree()).toContain('"name":"Theme light","disabled":true')
      expect(renderer.getAutomationTree()).toContain('"name":"Reduce motion","disabled":true')
      expect(renderer.getAutomationTree()).toContain('"name":"Focus Git shortcut","disabled":true')
      click("setting-theme-light")
      renderer.nativeSimulateKeystrokes(theme.id, "space")
      click("reduced-motion")
      renderer.nativeSimulateKeystrokes(motion.id, "space")
      renderer.nativeSimulateInput(shortcut.id, "mod+g")
      expect(settings.appearance()).toEqual({ theme: "dark", contrast: "standard", fontScale: "standard", reducedMotion: false })
      expect(settings.shortcut("focus-git")).toBeUndefined()
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("close-settings")!.id, "enter")
      expect(closed).toBe(1)
    } finally { unmount() }
  })

  it("keeps settings controls native-operable when settings are healthy", () => {
    const settings = new DesktopSettings()
    const dispatcher = createCommandDispatcher(settings, () => ({ canStart: false, canCancel: false }), { openSettings() {}, openProject() {}, openDiagnostics() {}, openPlugins() {}, openBrowser() {}, focusPanel() {}, toggleTheme() {}, startRun() {}, cancelRun() {} })
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(<CommandSurface dispatcher={dispatcher} settings={settings} settingsOpen onSettingsOpenChange={() => {}} />)
      const theme = renderer.findByTestId("setting-theme-light")!
      const motion = renderer.findByTestId("reduced-motion")!
      const shortcut = renderer.findByTestId("shortcut-focus-git")!
      renderer.nativeSimulateKeystrokes(theme.id, "space")
      renderer.nativeSimulateKeystrokes(motion.id, "space")
      renderer.nativeSimulateKeystrokes(shortcut.id, "cmd-a")
      renderer.nativeSimulateInput(shortcut.id, "mod+g")
      expect(settings.appearance()).toEqual({ theme: "light", contrast: "standard", fontScale: "standard", reducedMotion: true })
      expect(settings.shortcut("focus-git")).toBe("mod+g")
    } finally { unmount() }
  })

  it("requires confirmation before resetting only the selected workspace layout", () => {
    const settings = new DesktopSettings()
    const dispatcher = createCommandDispatcher(settings, () => ({ canStart: false, canCancel: false }), { openSettings() {}, openProject() {}, openDiagnostics() {}, openPlugins() {}, openBrowser() {}, focusPanel() {}, toggleTheme() {}, startRun() {}, cancelRun() {} })
    const resets: string[] = []
    const { render, renderer, unmount } = createTestRoot()
    const click = (testId: string) => { const element = renderer.findByTestId(testId)!; const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id)!; renderer.nativeSimulateClick(x + width / 2, y + height / 2) }
    try {
      render(<CommandSurface dispatcher={dispatcher} settings={settings} workspaceId="workspace-reset" settingsOpen onSettingsOpenChange={() => {}} onResetWorkspaceLayout={() => resets.push("workspace-reset")} />)
      click("reset-workspace-layout")
      expect(renderer.findByTestId("confirm-reset-workspace-layout")).toBeDefined()
      expect(resets).toEqual([])
      click("confirm-reset-workspace-layout")
      expect(resets).toEqual(["workspace-reset"])
      expect(renderer.findByTestId("reset-workspace-layout")).toBeDefined()
    } finally { unmount() }
  })

  it("uses the mounted Workbench conversation route for composer commands, disabled accessibility, and focus restoration", () => {
    const calls: string[] = []
    const { render, renderer, unmount } = createTestRoot()
    const click = (testId: string) => { const element = renderer.findByTestId(testId)!; const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id)!; renderer.nativeSimulateClick(x + width / 2, y + height / 2) }
    try {
      function ComposerHarness() {
        const [objective, setObjective] = useState("/focus")
        const [canStart, setCanStart] = useState(true)
        const dispatcher = useMemo(() => createCommandDispatcher(new DesktopSettings(), () => ({ canStart, canCancel: !canStart }), {
          openSettings: () => calls.push("settings"), openProject: () => calls.push("project"), openDiagnostics: () => calls.push("diagnostics"), openPlugins: () => calls.push("plugins"),
          openBrowser: () => calls.push("browser"),
          focusPanel: (panel) => calls.push(`focus:${panel}`), toggleTheme: () => calls.push("theme"), startRun: () => { calls.push("start"); setCanStart(false) }, cancelRun: () => { calls.push("cancel"); setCanStart(true) },
        }), [canStart])
        const panels = [{ id: "conversation", label: "Conversation", content: <ConversationPanel
        title="Commands"
        selectedConversationId="session-command-gpu"
        transcriptRows={[]}
        transcriptLoading={false}
        hasOlderTranscript={false}
        loadingOlderTranscript={false}
        onLoadOlderTranscript={() => {}}
        messages={[]}
        objective={objective}
        attachments={[]}
        onObjectiveChange={setObjective}
        onRemoveAttachment={() => {}}
        commands={dispatcher}
        /> }]
        return <div style={{ width: 1000, height: 700 }}><Workbench workspaceId="workspace-command-conversation" presentation={workspacePresentation("workspace-command-conversation")} panels={panels} commands={dispatcher} /><text testId="composer-objective">{objective || "empty"}</text></div>
      }
      desktopSettings.saveLayout("workspace-command-conversation", { kind: "tabs", id: "root", panels: ["conversation"], active: "conversation" })
      render(<ComposerHarness />)
      expect(renderer.getAutomationTree()).toContain("Composer slash commands")
      const slashMenuItem = renderer.findByTestId("composer-command-focus-git")!
      expect(renderer.getAutomationTree()).toContain('"role":"menuitem"')
      renderer.nativeSimulateKeystrokes(slashMenuItem.id, "escape")
      expect(renderer.findByTestId("composer-slash-completion")).toBeUndefined()
      expect(renderer.getPaintedText()).toContain("empty")
      renderer.nativeSimulateInput(renderer.findByTestId("run-objective")!.id, "/focus")
      click("composer-command-focus-git")
      expect(calls).toEqual(["focus:git"])
      expect(renderer.getPaintedText()).toContain("empty")
      renderer.simulateKeystrokes("x")
      expect(renderer.getPaintedText()).toContain("x")
      expect(renderer.getAutomationTree()).toContain("Start run")
      expect(renderer.getAutomationTree()).toContain("Cancel run")
      click("start-run")
      expect(calls).toEqual(["focus:git", "start"])
      const cancel = renderer.findByTestId("cancel-run")!
      click("cancel-run")
      expect(calls).toEqual(["focus:git", "start", "cancel"])
      const start = renderer.findByTestId("start-run")!
      renderer.nativeSimulateKeystrokes(start.id, "enter")
      expect(calls).toEqual(["focus:git", "start", "cancel", "start"])
      renderer.nativeSimulateKeystrokes(cancel.id, "enter")
      expect(calls).toEqual(["focus:git", "start", "cancel", "start", "cancel"])
      renderer.nativeSimulateKeystrokes(renderer.findByTestId("start-run")!.id, "enter")
      expect(calls).toEqual(["focus:git", "start", "cancel", "start", "cancel", "start"])
      const composer = renderer.findByTestId("run-objective")!
      renderer.nativeSimulateKeystrokes(composer.id, "cmd-a")
      renderer.nativeSimulateInput(composer.id, "/start")
      const disabledSlash = renderer.findByTestId("composer-command-start-run")!
      expect(renderer.getAutomationTree()).toContain("disabled")
      click("composer-command-start-run")
      renderer.nativeSimulateKeystrokes(disabledSlash.id, "enter")
      expect(calls).toEqual(["focus:git", "start", "cancel", "start", "cancel", "start"])
      renderer.nativeSimulateKeystrokes(composer.id, "cmd-a")
      renderer.nativeSimulateInput(composer.id, "/cancel")
      const slashCancel = renderer.findByTestId("composer-command-cancel-run")!
      renderer.nativeSimulateKeystrokes(slashCancel.id, "enter")
      expect(calls).toEqual(["focus:git", "start", "cancel", "start", "cancel", "start", "cancel"])
      renderer.simulateKeystrokes("y")
      expect(renderer.getPaintedText()).toContain("y")
    } finally { unmount() }
  })
})
