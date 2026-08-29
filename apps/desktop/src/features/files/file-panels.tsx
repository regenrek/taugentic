import { VirtualList } from "@regenrek/gpuix-react"
import type { BoundedFileContent, NativeImagePreview, WorkspaceFileEntry } from "@taugentic/desktop-protocol"
import { Fragment, useMemo, useState } from "react"

import { palette } from "../../app/theme.js"
import { CopyTextButton } from "../../ui/copy-text-button.js"
import { Pressable } from "../../ui/pressable.js"

export type FilePanelState = {
  entries: readonly WorkspaceFileEntry[]
  treeTruncated: boolean
  treeLoading: boolean
  treeError?: string
  selectedPath?: string
  selectedEntry?: WorkspaceFileEntry
  selectedContent?: BoundedFileContent
  selectedImagePreview?: NativeImagePreview
  contentLoading: boolean
  contentError?: string
  draft: string
  dirty: boolean
  attached: boolean
  attachmentEnabled: boolean
  saving: boolean
  selectEntry(entry: WorkspaceFileEntry): void
  setDraft(value: string): void
  save(): void
  discard(): void
  toggleAttachment(): void
  openExternal(): void
  refreshTree(): void
  pdfPageIndex: number
  setPdfPageIndex(page: number): void
}

type PreviewSource = {
  label?: string
  content?: BoundedFileContent
  image?: NativeImagePreview
  loading: boolean
  error?: string
  pdfPageIndex: number
  setPdfPageIndex(page: number): void
}

function PanelFrame(props: { testId: string; title: string; actions?: React.ReactNode; children: React.ReactNode }) {
  return <div testId={props.testId} style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, minWidth: 0, backgroundColor: palette.canvas }}>
    <div style={{ display: "flex", alignItems: "center", gap: 8, minHeight: 40, paddingLeft: 12, paddingRight: 8, borderBottomWidth: 1, borderColor: palette.border }}>
      <text style={{ color: palette.text, fontSize: 12, fontWeight: 650 }}>{props.title}</text>
      <div style={{ flexGrow: 1 }} />
      {props.actions}
    </div>
    <div style={{ display: "flex", flexDirection: "column", flexGrow: 1, minHeight: 0, minWidth: 0 }}>{props.children}</div>
  </div>
}

function Action(props: { testId: string; label: string; disabled?: boolean; active?: boolean; onClick(): void }) {
  const enabled = !props.disabled
  return <Pressable testId={props.testId} name={props.label} disabled={!enabled} selected={props.active} onPress={props.onClick} style={{ padding: 6, borderRadius: 5, cursor: enabled ? "pointer" : "default", color: enabled ? palette.text : palette.textFaint, backgroundColor: props.active ? palette.accentDim : palette.panelRaised }}><text style={{ fontSize: 10 }}>{props.label}</text></Pressable>
}

function Message(props: { children: string; error?: boolean }) {
  return <div style={{ padding: 16 }}><text style={{ color: props.error ? "#F08080" : palette.textMuted, fontSize: 11 }}>{props.children}</text></div>
}

function parentDirectories(path: string): string[] {
  const segments = path.split("/")
  return segments.slice(0, -1).map((_, index) => segments.slice(0, index + 1).join("/"))
}

export function FileTreePanel(props: FilePanelState) {
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => new Set())
  const visibleEntries = useMemo(() => props.entries.filter((entry) => (
    parentDirectories(entry.path).every((directory) => !collapsed.has(directory))
  )), [collapsed, props.entries])
  const toggleDirectory = (path: string) => setCollapsed((current) => {
    const next = new Set(current)
    if (next.has(path)) next.delete(path)
    else next.add(path)
    return next
  })
  return <PanelFrame testId="files-panel" title="Files" actions={<Action testId="refresh-files" label="Refresh" onClick={props.refreshTree} />}>
    {props.treeLoading && <Message>Loading project files…</Message>}
    {props.treeError && <Message error>{props.treeError}</Message>}
    {!props.treeLoading && !props.treeError && !visibleEntries.length && <Message>No project files.</Message>}
    {!!visibleEntries.length && <div testId="workspace-file-tree" style={{ display: "flex", flexGrow: 1, minHeight: 0, width: "100%" }}><VirtualList
      itemCount={visibleEntries.length}
      estimatedItemHeight={28}
      renderItem={(index) => {
        const entry = visibleEntries[index]
        if (!entry) return null
        const directory = entry.kind === "directory"
        const selected = props.selectedPath === entry.path
        const depth = Math.max(0, entry.path.split("/").length - 1)
        return <Fragment key={entry.path}><Pressable
          testId={`file-${entry.path}`}
          name={`${directory ? "Folder" : "File"} ${entry.path}`}
          role={directory ? "button" : "option"}
          disabled={entry.isSymlink}
          selected={selected}
          expanded={directory ? !collapsed.has(entry.path) : undefined}
          onPress={() => directory ? toggleDirectory(entry.path) : props.selectEntry(entry)}
          style={{ display: "flex", alignItems: "center", minHeight: 28, paddingLeft: 8 + depth * 14, paddingRight: 8, cursor: entry.isSymlink ? "default" : "pointer", backgroundColor: selected ? palette.panelRaised : palette.canvas, color: entry.isSymlink ? palette.textFaint : palette.text }}
        >
          <text style={{ width: 18, color: palette.textMuted }}>{directory ? (collapsed.has(entry.path) ? "▸" : "▾") : entry.kind === "image" ? "◫" : entry.kind === "pdf" ? "PDF" : "·"}</text>
          <text style={{ fontSize: 11 }}>{entry.name}</text>
          {entry.isSymlink && <text style={{ marginLeft: 8, color: palette.warning, fontSize: 9 }}>symlink</text>}
        </Pressable></Fragment>
      }}
      style={{ flexGrow: 1, minHeight: 0, width: "100%" }}
    /></div>}
    {props.treeTruncated && <Message>File limit reached. Refine the workspace before continuing.</Message>}
  </PanelFrame>
}

export function SourcePanel(props: FilePanelState) {
  const [editing, setEditing] = useState(false)
  const content = props.selectedContent
  const text = content?.kind === "text" ? content : undefined
  const title = props.selectedPath ? `Source · ${props.selectedPath}` : "Source"
  return <PanelFrame testId="source-panel" title={title} actions={<>
    <Action testId="attach-file" label={props.attached ? "Detach" : "Attach"} disabled={!props.selectedPath || !props.attachmentEnabled} active={props.attached} onClick={props.toggleAttachment} />
    <Action testId="open-file-external" label="Open externally" disabled={!props.selectedPath} onClick={props.openExternal} />
    <Action testId="toggle-file-edit" label={editing ? "Preview" : "Edit"} disabled={!text} active={editing} onClick={() => setEditing((value) => !value)} />
  </>}>
    {props.contentLoading && <Message>Loading file…</Message>}
    {props.contentError && <Message error>{props.contentError}</Message>}
    {!props.selectedPath && <Message>Select a text file.</Message>}
    {props.selectedPath && !props.contentLoading && !props.contentError && !text && <Message>This file is available in its matching preview panel.</Message>}
    {text && editing && <>
      <textarea testId="file-editor" value={props.draft} onChange={(event) => props.setDraft(event.value ?? "")} style={{ flexGrow: 1, minHeight: 0, width: "100%", padding: 12, borderWidth: 0, color: palette.text, backgroundColor: palette.canvas }} />
      <div style={{ display: "flex", gap: 8, alignItems: "center", minHeight: 44, padding: 8, borderTopWidth: 1, borderColor: palette.border }}>
        <Action testId="save-file" label={props.saving ? "Saving…" : "Save"} disabled={!props.dirty || props.saving} onClick={props.save} />
        <Action testId="discard-file" label="Discard" disabled={!props.dirty || props.saving} onClick={props.discard} />
        {props.dirty && <text style={{ color: palette.warning, fontSize: 10 }}>Unsaved changes</text>}
      </div>
    </>}
    {text && !editing && <div style={{ flexGrow: 1, minHeight: 0, overflow: "scroll" }}><code code={text.text} language={text.language ?? undefined} path={props.selectedPath} showLineNumbers showHeader={false} style={{ minWidth: "100%" }} /></div>}
  </PanelFrame>
}

export function DiffPanel(props: PreviewSource & { copyText?(text: string): void }) {
  const patch = props.content?.kind === "text" ? props.content.text : undefined
  return <PanelFrame testId="diff-panel" title={props.label ? `Diff · ${props.label}` : "Diff"} actions={patch ? <CopyTextButton testId="copy-selected-diff" text={patch} copyText={props.copyText} label="Copy patch" /> : undefined}>
    {props.loading && <Message>Loading diff…</Message>}
    {props.error && <Message error>{props.error}</Message>}
    {!props.loading && !props.error && !patch && <Message>Select a patch or diff.</Message>}
    {patch && <diff patch={patch} wordDiff scroll style={{ flexGrow: 1, minHeight: 0, width: "100%" }} />}
  </PanelFrame>
}

export function ImagePanel(props: PreviewSource) {
  const image = props.image
  return <PanelFrame testId="image-panel" title={props.label ? `Image · ${props.label}` : "Image"}>
    {props.loading && <Message>Loading image…</Message>}
    {props.error && <Message error>{props.error}</Message>}
    {!props.loading && !props.error && !image && <Message>Select an image.</Message>}
    {image && <div style={{ display: "flex", flexGrow: 1, minHeight: 0, alignItems: "center", justifyContent: "center", overflow: "scroll", padding: 12 }}><img src={image.source} alt={props.label ?? "Selected image"} objectFit="contain" style={{ width: "100%", height: "100%" }} /></div>}
  </PanelFrame>
}

export function PdfPanel(props: PreviewSource & { onOpenExternal?: () => void }) {
  const pdf = props.content?.kind === "pdf" ? props.content : undefined
  const previous = () => props.setPdfPageIndex(Math.max(0, props.pdfPageIndex - 1))
  const next = () => {
    if (!pdf) return
    props.setPdfPageIndex(Math.min(pdf.pageCount - 1, props.pdfPageIndex + 1))
  }
  return <PanelFrame testId="pdf-panel" title={props.label ? `PDF · ${props.label}` : "PDF"} actions={<>
    <Action testId="pdf-previous" label="Previous" disabled={!pdf || props.pdfPageIndex <= 0} onClick={previous} />
    <Action testId="pdf-next" label="Next" disabled={!pdf || props.pdfPageIndex + 1 >= pdf.pageCount} onClick={next} />
    {props.onOpenExternal && <Action testId="open-pdf-external" label="Open externally" disabled={!pdf} onClick={props.onOpenExternal} />}
  </>}>
    {props.loading && <Message>Rendering PDF page…</Message>}
    {props.error && <Message error>{props.error}</Message>}
    {!props.loading && !props.error && !pdf && <Message>Select a PDF.</Message>}
    {pdf && <>
      <div style={{ display: "flex", justifyContent: "center", minHeight: 28, padding: 6 }}><text style={{ color: palette.textMuted, fontSize: 10 }}>Page {pdf.pageIndex + 1} of {pdf.pageCount}</text></div>
      <div style={{ display: "flex", flexGrow: 1, minHeight: 0, alignItems: "flex-start", justifyContent: "center", overflow: "scroll", padding: 12 }}><img src={pdf.previewDataUri} alt={`PDF page ${pdf.pageIndex + 1}`} objectFit="contain" style={{ width: "100%", minHeight: 1 }} /></div>
    </>}
  </PanelFrame>
}
