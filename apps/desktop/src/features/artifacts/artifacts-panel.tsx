import { VirtualList } from "@regenrek/gpuix-react"
import type { ArtifactId, ArtifactSummary } from "@taugentic/desktop-protocol"
import { Fragment } from "react"

import { palette } from "../../app/theme.js"

export type ArtifactPanelState = {
  artifacts: readonly ArtifactSummary[]
  selectedArtifact?: ArtifactSummary
  loading: boolean
  error?: string
  selectArtifact(artifactId: ArtifactId): void
  openImageArtifact(artifactId: ArtifactId): void
  refresh(): void
}

export function ArtifactsPanel(props: ArtifactPanelState) {
  return <div testId="artifacts-panel" style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, backgroundColor: palette.canvas }}>
    <div style={{ display: "flex", alignItems: "center", minHeight: 40, paddingLeft: 12, paddingRight: 8, borderBottomWidth: 1, borderColor: palette.border }}>
      <text style={{ color: palette.text, fontSize: 12, fontWeight: 650 }}>Artifacts</text><div style={{ flexGrow: 1 }} />
      <div testId="refresh-artifacts" tabIndex={0} onClick={props.refresh} style={{ padding: 6, borderRadius: 5, cursor: "pointer", backgroundColor: palette.panelRaised }}><text style={{ fontSize: 10 }}>Refresh</text></div>
    </div>
    {props.loading && <div style={{ padding: 16 }}><text style={{ color: palette.textMuted }}>Loading artifacts…</text></div>}
    {props.error && <div style={{ padding: 16 }}><text style={{ color: "#F08080" }}>{props.error}</text></div>}
    {!props.loading && !props.error && !props.artifacts.length && <div style={{ padding: 16 }}><text style={{ color: palette.textMuted }}>No generated artifacts.</text></div>}
    {!!props.artifacts.length && <div testId="artifact-list" style={{ display: "flex", flexGrow: 1, minHeight: 0, width: "100%" }}><VirtualList
      itemCount={props.artifacts.length}
      estimatedItemHeight={44}
      renderItem={(index) => {
        const artifact = props.artifacts[index]
        if (!artifact) return null
        const selected = artifact.id === props.selectedArtifact?.id
        return <Fragment key={artifact.id}><div testId={`artifact-${artifact.id}`} tabIndex={0} accessibilitySelected={selected} onClick={() => props.selectArtifact(artifact.id)} style={{ display: "flex", flexDirection: "column", gap: 3, minHeight: 44, padding: 8, cursor: "pointer", backgroundColor: selected ? palette.panelRaised : palette.canvas }}>
          <text style={{ color: palette.text, fontSize: 11 }}>{artifact.displayName}</text>
          <text style={{ color: palette.textMuted, fontSize: 9 }}>{artifact.kind}</text>
          {artifact.kind === "Image" && <div testId={`open-image-artifact-${artifact.id}`} tabIndex={0} onClick={() => props.openImageArtifact(artifact.id)} style={{ alignSelf: "flex-start", padding: 4, borderRadius: 4, cursor: "pointer", backgroundColor: palette.accentDim }}><text style={{ fontSize: 9 }}>Open</text></div>}
        </div></Fragment>
      }}
      style={{ flexGrow: 1, minHeight: 0, width: "100%" }}
    /></div>}
  </div>
}

export function artifactDisplayName(artifact?: ArtifactSummary): string | undefined {
  return artifact?.displayName
}
