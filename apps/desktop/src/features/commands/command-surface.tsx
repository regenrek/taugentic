import { Combobox, ComboboxContent, ComboboxEmpty, ComboboxInput, ComboboxItem, ComboboxList, useGpuixRequired } from "@regenrek/gpuix-react"
import { Fragment, type RefObject, useMemo, useRef, useState } from "react"

import { fontSize, palette } from "../../app/theme.js"
import type { DesktopSettings } from "../../platform/settings/desktop-settings.js"
import { Pressable } from "../../ui/pressable.js"
import { commandRegistry, type CommandDispatcher, type CommandId } from "./registry.js"

export function CommandSurface({ dispatcher, settings, workspaceId, settingsOpen, onSettingsOpenChange, onResetWorkspaceLayout }: {
  dispatcher: CommandDispatcher
  settings: DesktopSettings
  workspaceId?: string
  settingsOpen: boolean
  onSettingsOpenChange(open: boolean): void
  onResetWorkspaceLayout?(): void
}) {
  const [paletteOpen, setPaletteOpen] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)
  const [query, setQuery] = useState("")
  const [paletteActiveId, setPaletteActiveId] = useState<CommandId | undefined>()
  const [shortcutFeedback, setShortcutFeedback] = useState("")
  const renderer = useGpuixRequired()
  const paletteTrigger = useRef<any>(null)
  const menuTrigger = useRef<any>(null)
  const settingsTrigger = useRef<any>(null)
  const commands = useMemo(() => commandRegistry.filter((command) => command.title.toLowerCase().includes(query.toLowerCase())), [query])
  const enabledCommands = commands.filter((command) => dispatcher.enabled(command.id))
  const activeCommand = enabledCommands.find((command) => command.id === paletteActiveId) ?? enabledCommands[0]
  const close = (setOpen: (open: boolean) => void, trigger: RefObject<any>) => {
    setOpen(false)
    if (trigger.current) renderer.focusElement?.(trigger.current.id)
  }
  const openPalette = () => {
    setQuery("")
    setPaletteActiveId(commandRegistry.find((command) => dispatcher.enabled(command.id))?.id)
    setPaletteOpen(true)
  }
  const updatePaletteQuery = (value: string) => {
    setQuery(value)
    setPaletteActiveId(commandRegistry.find((command) => command.title.toLowerCase().includes(value.toLowerCase()) && dispatcher.enabled(command.id))?.id)
  }
  const movePaletteActive = (offset: number) => {
    if (!enabledCommands.length) return
    const current = enabledCommands.findIndex((command) => command.id === activeCommand?.id)
    setPaletteActiveId(enabledCommands[(current + offset + enabledCommands.length) % enabledCommands.length]!.id)
  }
  const invoke = (id: CommandId, trigger = paletteTrigger) => {
    dispatcher.dispatch(id)
    setQuery("")
    close(setPaletteOpen, trigger)
  }
  const openSettings = () => { dispatcher.dispatch("open-settings") }

  return <div testId="command-surface" style={{ display: "flex", alignItems: "center", gap: 8 }}>
    <Pressable ref={paletteTrigger} testId="command-palette-toggle" name="Open command palette" expanded={paletteOpen} onPress={() => paletteOpen ? close(setPaletteOpen, paletteTrigger) : openPalette()} style={triggerStyle()}><text>Palette</text></Pressable>
    <Pressable ref={menuTrigger} testId="command-menu" name="Open command menu" expanded={menuOpen} onPress={() => setMenuOpen((open) => !open)} style={triggerStyle()}><text>Commands</text></Pressable>
    <Pressable ref={settingsTrigger} testId="settings-toggle" name="Open settings" expanded={settingsOpen} onPress={openSettings} style={triggerStyle()}><text>Settings</text></Pressable>
    {paletteOpen && <Combobox open inputValue={query} onInputValueChange={updatePaletteQuery} onOpenChange={(open) => { if (!open) close(setPaletteOpen, paletteTrigger) }} items={commands.map((command) => command.id)} filter={null}><ComboboxInput testId="command-palette-input" placeholder="Search commands" accessibilityName="Command palette" onKeyDown={(event) => { if (event.key === "down") movePaletteActive(1); if (event.key === "up") movePaletteActive(-1) }} onSubmit={() => { if (activeCommand) invoke(activeCommand.id) }} style={inputStyle()} /><ComboboxContent side="bottom" onMouseDownOutside={() => close(setPaletteOpen, paletteTrigger)} style={popupStyle(300)}><ComboboxList>{(id) => { const command = commandRegistry.find((candidate) => candidate.id === id as CommandId)!; const enabled = dispatcher.enabled(command.id); return <ComboboxItem key={id} value={id} disabled={!enabled} accessibilityRole="menuitem" accessibilityName={command.title} accessibilitySelected={activeCommand?.id === command.id} accessibilityDisabled={!enabled} onClick={() => invoke(command.id)} style={{ padding: 8, backgroundColor: palette.panelRaised }}><text>{command.title} · {dispatcher.shortcutFor(command.id) ?? ""}</text></ComboboxItem> }}</ComboboxList>{!commands.length && <ComboboxEmpty><text>No commands</text></ComboboxEmpty>}</ComboboxContent></Combobox>}
    {menuOpen && <Combobox open onOpenChange={(open) => { if (!open) close(setMenuOpen, menuTrigger) }} items={[]}><ComboboxContent testId="visible-command-menu" side="bottom" onMouseDownOutside={() => close(setMenuOpen, menuTrigger)} onKeyDown={(event) => { if (event.key === "escape") close(setMenuOpen, menuTrigger) }} style={popupStyle(260)}>{commandRegistry.map((command) => <Fragment key={command.id}><Pressable testId={`visible-command-${command.id}`} role="menuitem" name={command.title} disabled={!dispatcher.enabled(command.id)} onPress={() => { if (dispatcher.dispatch(command.id)) close(setMenuOpen, menuTrigger) }} style={{ padding: 8, backgroundColor: palette.panelRaised, cursor: dispatcher.enabled(command.id) ? "pointer" : "default" }}><text>{command.title} · {dispatcher.shortcutFor(command.id) ?? ""}</text></Pressable></Fragment>)}</ComboboxContent></Combobox>}
    {settingsOpen && <SettingsSurface settings={settings} dispatcher={dispatcher} workspaceId={workspaceId} shortcutFeedback={shortcutFeedback} setShortcutFeedback={setShortcutFeedback} onResetWorkspaceLayout={onResetWorkspaceLayout} onClose={() => { onSettingsOpenChange(false); if (settingsTrigger.current) renderer.focusElement?.(settingsTrigger.current.id) }} />}
  </div>
}

function SettingsSurface({ settings, dispatcher, workspaceId, shortcutFeedback, setShortcutFeedback, onResetWorkspaceLayout, onClose }: {
  settings: DesktopSettings
  dispatcher: CommandDispatcher
  workspaceId?: string
  shortcutFeedback: string
  setShortcutFeedback(value: string): void
  onResetWorkspaceLayout?(): void
  onClose(): void
}) {
  const appearance = settings.appearance()
  const mutable = settings.error() === undefined
  const [resetPending, setResetPending] = useState(false)
  return <Combobox open onOpenChange={(open) => { if (!open) onClose() }} items={[]}><ComboboxContent testId="desktop-settings" side="bottom" onMouseDownOutside={onClose} style={popupStyle(360)} accessibilityRole="dialog" accessibilityName="Desktop settings">
    <div style={{ display: "flex", alignItems: "center", gap: 8, padding: 4 }}><text style={{ color: palette.text, fontWeight: 650 }}>Settings</text><div style={{ flexGrow: 1 }} /><Pressable testId="close-settings" name="Close settings" onPress={onClose} style={triggerStyle()}><text>Close</text></Pressable></div>
    {settings.error() && <text testId="desktop-settings-error" accessibilityRole="alert" accessibilityName={settings.error()} style={{ color: "#f08080", padding: 4 }}>{settings.error()}</text>}
    <SettingChoice label="Theme" value={appearance.theme} choices={["dark", "light"] as const} disabled={!mutable} onChange={(theme) => settings.saveAppearance({ theme })} />
    <SettingChoice label="Contrast" value={appearance.contrast} choices={["standard", "high"] as const} disabled={!mutable} onChange={(contrast) => settings.saveAppearance({ contrast })} />
    <SettingChoice label="Font scale" value={appearance.fontScale} choices={["standard", "large"] as const} disabled={!mutable} onChange={(fontScale) => settings.saveAppearance({ fontScale })} />
    <Pressable testId="reduced-motion" role="checkbox" name="Reduce motion" checked={appearance.reducedMotion} disabled={!mutable} onPress={() => settings.saveAppearance({ reducedMotion: !appearance.reducedMotion })} style={{ display: "flex", gap: 8, padding: 7, ...mutableControlStyle(mutable) }}><text>{appearance.reducedMotion ? "✓" : "○"} Reduce motion</text></Pressable>
    {workspaceId && onResetWorkspaceLayout && <div style={{ display: "flex", gap: 6, padding: 4 }}>{resetPending
      ? <><Pressable testId="confirm-reset-workspace-layout" name="Confirm reset workspace layout" disabled={!mutable} onPress={() => { if (mutable) { onResetWorkspaceLayout(); setResetPending(false) } }} style={mutableControlStyle(mutable)}><text>Confirm reset layout</text></Pressable><Pressable testId="cancel-reset-workspace-layout" name="Cancel reset workspace layout" onPress={() => setResetPending(false)} style={triggerStyle()}><text>Cancel</text></Pressable></>
      : <Pressable testId="reset-workspace-layout" name="Reset workspace layout" disabled={!mutable} onPress={() => { if (mutable) setResetPending(true) }} style={mutableControlStyle(mutable)}><text>Reset workspace layout</text></Pressable>
    }</div>}
    <text style={{ color: palette.textMuted, padding: 4, fontSize: fontSize(11) }}>Global shortcuts</text>
    {commandRegistry.map((command) => <Fragment key={command.id}><div style={{ display: "flex", gap: 8, padding: 3 }}><text style={{ width: 160 }}>{command.title}</text><input testId={`shortcut-${command.id}`} tabIndex={mutable ? 0 : -1} readOnly={!mutable} accessibilityRole="textbox" accessibilityName={`${command.title} shortcut`} accessibilityDisabled={!mutable} value={dispatcher.shortcutFor(command.id) ?? ""} onKeyDown={(event) => { if (mutable && event.key === "escape") onClose() }} onChange={(event) => { if (mutable) setShortcutFeedback(settings.saveShortcut(command.id, event.value ?? "") === "conflict" ? "Shortcut is already assigned." : "") }} style={inputStyle(mutable)} /></div></Fragment>)}
    {!!shortcutFeedback && <text testId="shortcut-conflict" accessibilityRole="alert" accessibilityName={shortcutFeedback}>{shortcutFeedback}</text>}
  </ComboboxContent></Combobox>
}

function SettingChoice<T extends string>({ label, value, choices, disabled, onChange }: { label: string; value: T; choices: readonly T[]; disabled: boolean; onChange(value: T): void }) {
  return <div style={{ display: "flex", gap: 5, padding: 4 }}><text style={{ width: 100, color: palette.textMuted }}>{label}</text>{choices.map((choice) => <Fragment key={choice}><Pressable testId={`setting-${label.toLowerCase().replace(" ", "-")}-${choice}`} role="radio" name={`${label} ${choice}`} checked={value === choice} disabled={disabled} onPress={() => onChange(choice)} style={{ padding: 5, backgroundColor: value === choice ? palette.accentDim : palette.panel, ...mutableControlStyle(!disabled) }}><text>{choice}</text></Pressable></Fragment>)}</div>
}

function triggerStyle() { return { cursor: "pointer", padding: 6, backgroundColor: palette.panelRaised } }
function mutableControlStyle(mutable: boolean) { return { cursor: mutable ? "pointer" : "default", pointerEvents: mutable ? "auto" as const : "none" as const, color: mutable ? palette.text : palette.textFaint } }
function inputStyle(mutable = true) { return { width: 110, padding: 7, color: mutable ? palette.text : palette.textFaint, backgroundColor: palette.panel, borderWidth: 1, borderColor: palette.border, cursor: mutable ? "text" : "default", pointerEvents: mutable ? "auto" as const : "none" as const } }
function popupStyle(width: number) { return { width, padding: 8, backgroundColor: palette.panelRaised, borderWidth: 1, borderColor: palette.border } }
