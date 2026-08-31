import { useGpuixRequired } from "@regenrek/gpuix-react"
import { Fragment, useState } from "react"

import { fontSize, palette } from "../../app/theme.js"
import type { PluginsState } from "./use-plugins.js"

function activates(event: { key?: string }): boolean { return event.key === "enter" || event.key === "space" }

function actionStyle(enabled: boolean) {
  return { cursor: enabled ? "pointer" : "default", padding: 7, backgroundColor: enabled ? palette.panelRaised : palette.panel, opacity: enabled ? 1 : 0.6 }
}

function capabilityLabel(capability: string): string {
  return capability.replace(/([A-Z])/g, " $1").replace(/^./, (letter) => letter.toUpperCase())
}

/** Global installation review for daemon-owned Plugins. Package code cannot run from this surface. */
export function PluginsPanel({ plugins, onClose }: { plugins: PluginsState; onClose(): void }) {
  const renderer = useGpuixRequired()
  const [chooserError, setChooserError] = useState<string>()
  const choosePackage = async () => {
    try {
      const sourcePath = await renderer.promptForDirectory()
      if (sourcePath !== null) plugins.inspect(sourcePath)
    } catch {
      setChooserError("The package chooser could not be opened.")
    }
  }
  const chooseEnabled = !plugins.busy
  const installEnabled = Boolean(plugins.inspection) && !plugins.busy

  return <div testId="plugins-panel" accessibilityRole="dialog" accessibilityName="Plugins" style={{ position: "absolute", right: 18, top: 50, width: 420, maxHeight: 740, overflow: "scroll", display: "flex", flexDirection: "column", gap: 10, padding: 12, backgroundColor: palette.panel, borderWidth: 1, borderColor: palette.border, borderRadius: 8 }}>
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <text style={{ color: palette.text, fontSize: fontSize(15), fontWeight: 700 }}>Plugins</text><div style={{ flexGrow: 1 }} />
      <div testId="close-plugins" tabIndex={0} accessibilityRole="button" accessibilityName="Close plugins" onClick={onClose} onKeyDown={(event) => { if (activates(event)) onClose() }} style={actionStyle(true)}><text>Close</text></div>
    </div>
    <text style={{ color: palette.textMuted, fontSize: fontSize(11) }}>Installed packages are disabled. This screen cannot activate or run Plugin code.</text>
    <div testId="choose-plugin-package" tabIndex={chooseEnabled ? 0 : -1} accessibilityRole="button" accessibilityName="Choose Plugin package" accessibilityDisabled={!chooseEnabled} onClick={() => { if (chooseEnabled) void choosePackage() }} onKeyDown={(event) => { if (activates(event) && chooseEnabled) void choosePackage() }} style={actionStyle(chooseEnabled)}><text>Choose package</text></div>
    {plugins.error && <text testId="plugins-error" accessibilityRole="alert" style={{ color: "#f08080", fontSize: fontSize(11) }}>{plugins.error}</text>}
    {plugins.mutationError && <text testId="plugins-mutation-error" accessibilityRole="alert" style={{ color: "#f08080", fontSize: fontSize(11) }}>{plugins.mutationError}</text>}
    {chooserError && <text testId="plugin-chooser-error" accessibilityRole="alert" style={{ color: "#f08080", fontSize: fontSize(11) }}>{chooserError}</text>}
    {plugins.inspection && <div testId="plugin-inspection" style={{ display: "flex", flexDirection: "column", gap: 7, padding: 9, borderWidth: 1, borderColor: palette.border, borderRadius: 7, backgroundColor: palette.canvas }}>
      <text style={{ color: palette.text, fontWeight: 700 }}>{plugins.inspection.pluginId}</text>
      <text testId="plugin-inspection-version" style={{ color: palette.textMuted, fontSize: fontSize(11) }}>Version {plugins.inspection.version}</text>
      <text testId="plugin-inspection-digest" style={{ color: palette.textMuted, fontSize: fontSize(10) }}>{plugins.inspection.digestSha256}</text>
      <text style={{ color: palette.textFaint, fontSize: fontSize(10), fontWeight: 700 }}>GRANT CAPABILITIES</text>
      {!plugins.inspection.requestedCapabilities.length && <text testId="plugin-no-requested-capabilities" style={{ color: palette.textMuted, fontSize: fontSize(11) }}>This package requests no capabilities.</text>}
      {plugins.inspection.requestedCapabilities.map((capability) => {
        const granted = plugins.grantedCapabilities.includes(capability)
        return <Fragment key={capability}><div testId={`plugin-capability-${capability}`} tabIndex={plugins.busy ? -1 : 0} accessibilityRole="checkbox" accessibilityName={`Grant ${capabilityLabel(capability)}`} accessibilityChecked={granted} accessibilityDisabled={plugins.busy} onClick={() => { if (!plugins.busy) plugins.setGranted(capability, !granted) }} onKeyDown={(event) => { if (activates(event) && !plugins.busy) plugins.setGranted(capability, !granted) }} style={actionStyle(!plugins.busy)}><text>{granted ? "✓ " : "○ "}{capabilityLabel(capability)}</text></div></Fragment>
      })}
      <div style={{ display: "flex", gap: 7 }}>
        <div testId="confirm-plugin-install" tabIndex={installEnabled ? 0 : -1} accessibilityRole="button" accessibilityName="Confirm Plugin installation" accessibilityDisabled={!installEnabled} onClick={() => { if (installEnabled) plugins.install() }} onKeyDown={(event) => { if (activates(event) && installEnabled) plugins.install() }} style={actionStyle(installEnabled)}><text>Install disabled</text></div>
        <div testId="cancel-plugin-install" tabIndex={plugins.busy ? -1 : 0} accessibilityRole="button" accessibilityName="Cancel Plugin installation" accessibilityDisabled={plugins.busy} onClick={() => plugins.clearInspection()} onKeyDown={(event) => { if (activates(event) && !plugins.busy) plugins.clearInspection() }} style={actionStyle(!plugins.busy)}><text>Cancel</text></div>
      </div>
    </div>}
    <text style={{ color: palette.textFaint, fontSize: fontSize(10), fontWeight: 700 }}>INSTALLED</text>
    {plugins.loading && !plugins.installations.length && <text testId="plugins-loading" style={{ color: palette.textMuted, fontSize: fontSize(11) }}>Loading Plugins…</text>}
    {!plugins.loading && !plugins.installations.length && <text testId="plugins-empty" style={{ color: palette.textMuted, fontSize: fontSize(11) }}>No Plugins installed.</text>}
    {plugins.installations.map((installation) => {
      const identity = `${installation.pluginId}-${installation.version}-${installation.digestSha256}`
      return <Fragment key={identity}><div testId={`plugin-installation-${identity}`} style={{ display: "flex", flexDirection: "column", gap: 5, padding: 9, borderWidth: 1, borderColor: palette.border, borderRadius: 7, backgroundColor: palette.canvas }}>
      <text style={{ color: palette.text, fontWeight: 700 }}>{installation.pluginId}</text>
      <text style={{ color: palette.textMuted, fontSize: fontSize(11) }}>Version {installation.version}</text>
      <text testId={`plugin-lifecycle-${identity}`} style={{ color: palette.textMuted, fontSize: fontSize(11) }}>{installation.lifecycleState === "disabled" ? "Disabled" : installation.lifecycleState}</text>
      <text style={{ color: palette.textMuted, fontSize: fontSize(10) }}>{installation.digestSha256}</text>
      <div testId={`uninstall-plugin-${identity}`} tabIndex={plugins.busy ? -1 : 0} accessibilityRole="button" accessibilityName={`Uninstall Plugin ${installation.pluginId} ${installation.version} ${installation.digestSha256}`} accessibilityDisabled={plugins.busy} onClick={() => plugins.uninstall(installation)} onKeyDown={(event) => { if (activates(event) && !plugins.busy) plugins.uninstall(installation) }} style={actionStyle(!plugins.busy)}><text>Uninstall</text></div>
    </div></Fragment>
    })}
  </div>
}
