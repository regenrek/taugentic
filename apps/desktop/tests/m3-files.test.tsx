import { createTestRoot } from "@regenrek/gpuix-react/testing"
import type { DockLayout } from "@regenrek/gpuix-react"
import { describe, expect, it } from "bun:test"

import type { ArtifactSummary, WorkspaceFileEntry } from "@taugentic/desktop-protocol"

import { ArtifactsPanel } from "../src/features/artifacts/artifacts-panel.js"
import { DiffPanel, FileTreePanel, ImagePanel, PdfPanel, SourcePanel, type FilePanelState } from "../src/features/files/file-panels.js"
import { defaultWorkspaceLayout } from "../src/features/workspace-layout/layout-store.js"

function fileState(overrides: Partial<FilePanelState> = {}): FilePanelState {
  return {
    entries: [],
    treeTruncated: false,
    treeLoading: false,
    selectedContent: undefined,
    contentLoading: false,
    draft: "",
    dirty: false,
    attached: false,
    attachmentEnabled: true,
    saving: false,
    selectEntry: () => {},
    setDraft: () => {},
    save: () => {},
    discard: () => {},
    toggleAttachment: () => {},
    openExternal: () => {},
    refreshTree: () => {},
    pdfPageIndex: 0,
    setPdfPageIndex: () => {},
    ...overrides,
  }
}

function layoutPanels(layout: DockLayout): string[] {
  if (layout.kind === "tabs") return layout.panels
  return [...layoutPanels(layout.first), ...layoutPanels(layout.second)]
}

function click(renderer: ReturnType<typeof createTestRoot>["renderer"], testId: string) {
  const element = renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id) ?? []
  renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

describe("M3 files and artifact workbench", () => {
  it("places every workbench panel in one nested movable layout", () => {
    expect(layoutPanels(defaultWorkspaceLayout).sort()).toEqual([
      "activity",
      "artifacts",
      "conversation",
      "diff",
      "files",
      "git",
      "image",
      "pdf",
      "pull-requests",
      "source",
      "terminal",
      "thread-workspace",
    ])
  })

  it("virtualizes a large daemon-projected file tree", () => {
    const { render, renderer, unmount } = createTestRoot()
    const entries: WorkspaceFileEntry[] = Array.from({ length: 10_000 }, (_, index) => ({
      path: `src/file-${index}.ts`,
      name: `file-${index}.ts`,
      kind: "text",
      isSymlink: false,
      byteLen: "10",
    }))
    try {
      render(<FileTreePanel {...fileState({ entries })} />)
      const list = renderer.findByType("virtual-list")[0]!
      expect(list.children.length).toBeLessThan(entries.length)
    } finally {
      unmount()
    }
  })

  it("keeps source mutation behind an explicit edit and save action", () => {
    const { render, renderer, unmount } = createTestRoot()
    let saves = 0
    try {
      render(<SourcePanel {...fileState({
        selectedPath: "src/main.rs",
        selectedContent: { kind: "text", text: "fn main() {}", revision: "rev", language: "rust", byteLen: "12" },
        draft: "fn main() { println!(\"hi\"); }",
        dirty: true,
        save: () => { saves += 1 },
      })} />)
      expect(renderer.findByType("code")).toHaveLength(1)
      click(renderer, "toggle-file-edit")
      expect(renderer.findByTestId("file-editor")).toBeDefined()
      click(renderer, "save-file")
      expect(saves).toBe(1)
    } finally {
      unmount()
    }
  })

  it("attaches and detaches the selected daemon-revisioned file explicitly", () => {
    const { render, renderer, unmount } = createTestRoot()
    let toggles = 0
    try {
      render(<SourcePanel {...fileState({
        selectedPath: "src/main.rs",
        selectedContent: { kind: "text", text: "fn main() {}", revision: "rev", language: "rust", byteLen: "12" },
        toggleAttachment: () => { toggles += 1 },
      })} />)
      click(renderer, "attach-file")
      expect(toggles).toBe(1)
    } finally {
      unmount()
    }
  })

  it("mounts native diff, bridge-owned image, and paged PDF preview primitives", () => {
    const { render, renderer, unmount } = createTestRoot()
    try {
      render(<div style={{ display: "flex", width: 1200, height: 700 }}>
        <DiffPanel label="change.diff" content={{ kind: "text", text: "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-old\n+new\n", revision: "r1", language: "diff", byteLen: "70" }} loading={false} pdfPageIndex={0} setPdfPageIndex={() => {}} />
        <ImagePanel label="pixel.png" image={{ source: "/bridge-owned/pixel.png", mediaType: "image/png", revision: "r2", byteLen: "68" }} loading={false} pdfPageIndex={0} setPdfPageIndex={() => {}} />
        <PdfPanel label="report.pdf" content={{ kind: "pdf", previewDataUri: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=", pageIndex: 0, pageCount: 2, revision: "r3", byteLen: "100" }} loading={false} pdfPageIndex={0} setPdfPageIndex={() => {}} />
      </div>)
      expect(renderer.findByType("diff")).toHaveLength(1)
      expect(renderer.findByType("img")).toHaveLength(2)
      expect(renderer.findByTestId("pdf-next")).toBeDefined()
    } finally {
      unmount()
    }
  })

  it("opens a generated image artifact in the native Image panel", () => {
    const { render, renderer, unmount } = createTestRoot()
    const artifacts: ArtifactSummary[] = [{ id: "artifact-image", runId: "run-one", kind: "Image", metadata: { kind: "image", mediaType: "png", sha256: "abc", byteLen: "1", provenance: { runtimeProfileId: "profile", providerId: "provider", turnId: "turn", itemId: "item" } }, displayName: "render.png" }]
    let opened: string | undefined
    try {
      render(<ArtifactsPanel artifacts={artifacts} loading={false} selectArtifact={() => {}} openImageArtifact={(id) => { opened = id }} refresh={() => {}} />)
      click(renderer, "open-image-artifact-artifact-image")
      expect(opened).toBe("artifact-image")
    } finally {
      unmount()
    }
  })
})
