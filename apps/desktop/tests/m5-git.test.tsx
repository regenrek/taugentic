import { createTestRoot } from "@regenrek/gpuix-react/testing"
import { describe, expect, it } from "bun:test"

import type { GitRepositorySnapshot } from "@taugentic/desktop-protocol"

import { GitPanel } from "../src/features/git/git-panel.js"
import type { WorkbenchGitState } from "../src/features/git/use-workbench-git.js"
import { codeHostState } from "./support/code-host.js"

const snapshot: GitRepositorySnapshot = {
  branch: "feature/native-git",
  head: "0123456789012345678901234567890123456789",
  upstream: "origin/feature/native-git",
  ahead: 2,
  behind: 1,
  fingerprint: "sha256:test",
  truncated: false,
  files: [
    { path: "src/main.rs", staged: "modified", unstaged: "modified" },
    { path: "new file.txt", unstaged: "untracked" },
  ],
  worktrees: [
    { path: "/tmp/project", branch: "feature/native-git", head: "0123456789012345678901234567890123456789", current: true, locked: false },
  ],
}

function state(overrides: Partial<WorkbenchGitState> = {}): WorkbenchGitState {
  return {
    snapshot,
    visibleFiles: snapshot.files,
    view: "unstaged",
    setView: () => {},
    selectedPaths: [],
    togglePath: () => {},
    patch: "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new\n",
    patchLoading: false,
    preparedRevert: undefined,
    cancelRevert: () => {},
    checkpoints: [{
      checkpointId: "checkpoint-one",
      workspaceId: "workspace-one",
      runId: "run-one",
      revision: "1",
      phase: "afterTurn",
      createdAtMs: "1",
    }],
    commitMessage: "",
    setCommitMessage: () => {},
    busy: false,
    loading: false,
    error: undefined,
    canStage: false,
    canUnstage: false,
    canCommit: false,
    stageSelected: () => {},
    unstageSelected: () => {},
    commit: () => {},
    prepareRevert: () => {},
    applyRevert: () => {},
    refresh: () => {},
    ...overrides,
  }
}

function click(renderer: ReturnType<typeof createTestRoot>["renderer"], testId: string) {
  const element = renderer.findByTestId(testId)!
  const [x = 0, y = 0, width = 0, height = 0] = renderer.getElementBounds(element.id) ?? []
  renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

describe("M5 native Git workbench", () => {
  it("renders daemon-owned repository state, worktrees, checkpoints, and the native diff", () => {
    const root = createTestRoot()
    try {
      root.render(<div style={{ width: 1200, height: 760 }}><GitPanel git={state()} codeHost={codeHostState()} /></div>)
      expect(root.renderer.getPaintedText()).toContain("feature/native-git")
      expect(root.renderer.getPaintedText()).toContain("origin/feature/native-git")
      expect(root.renderer.getPaintedText()).toContain("After turn")
      expect(root.renderer.findByType("diff")).toHaveLength(1)
    } finally {
      root.unmount()
    }
  })

  it("requires explicit file selection before staging", () => {
    const root = createTestRoot()
    const selected: string[] = []
    let stageCount = 0
    try {
      root.render(<div style={{ width: 1200, height: 760 }}><GitPanel git={state({
        canStage: true,
        selectedPaths: ["src/main.rs"],
        togglePath: (path) => selected.push(path),
        stageSelected: () => { stageCount += 1 },
      })} codeHost={codeHostState()} /></div>)
      click(root.renderer, "git-file-src/main.rs")
      click(root.renderer, "git-stage-selected")
      expect(selected).toEqual(["src/main.rs"])
      expect(stageCount).toBe(1)
    } finally {
      root.unmount()
    }
  })

  it("exposes checkpoint restore only after the exact patch was prepared", () => {
    const root = createTestRoot()
    let applyCount = 0
    try {
      root.render(<div style={{ width: 1200, height: 760 }}><GitPanel git={state()} codeHost={codeHostState()} /></div>)
      expect(root.renderer.findByTestId("apply-git-revert")).toBeUndefined()

      root.render(<div style={{ width: 1200, height: 760 }}><GitPanel git={state({
        preparedRevert: {
          token: "opaque-token",
          patch: "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-new\n+old\n",
          checkpoint: {
            checkpointId: "checkpoint-one",
            workspaceId: "workspace-one",
            runId: "run-one",
            revision: "0",
            phase: "beforeTurn",
            createdAtMs: "1",
          },
          currentFingerprint: "sha256:current",
        },
        applyRevert: () => { applyCount += 1 },
      })} codeHost={codeHostState()} /></div>)
      click(root.renderer, "apply-git-revert")
      expect(applyCount).toBe(1)
    } finally {
      root.unmount()
    }
  })

  it("virtualizes large repository status lists", () => {
    const root = createTestRoot()
    const files = Array.from({ length: 20_000 }, (_, index) => ({
      path: `src/file-${index}.rs`,
      unstaged: "modified" as const,
    }))
    try {
      root.render(<div style={{ width: 1200, height: 760 }}><GitPanel git={state({ visibleFiles: files })} codeHost={codeHostState()} /></div>)
      const list = root.renderer.findByType("virtual-list")[0]!
      expect(list.children.length).toBeLessThan(files.length)
    } finally {
      root.unmount()
    }
  })

  it("requires an exact push preview before one non-force apply action", () => {
    const root = createTestRoot()
    let applies = 0
    try {
      root.render(<div style={{ width: 1200, height: 760 }}><GitPanel git={state()} codeHost={codeHostState({
        mutationBusy: false,
        preparedPush: {
          token: "opaque-one-shot-token",
          remote: {
            remoteName: "origin",
            repository: { provider: "gitHub", host: "github.com", owner: "example-owner", name: "example-project" },
          },
          sourceHead: "1111111111111111111111111111111111111111",
          destinationBranch: "release/native",
          remoteHead: "2222222222222222222222222222222222222222",
          commits: [{ id: "1111111111111111111111111111111111111111", subject: "Ship native delivery" }],
          truncated: false,
        },
        applyPush: () => { applies += 1 },
      })} /></div>)
      const painted = root.renderer.getPaintedText().join("")
      expect(painted).toContain("Push 1 commit to origin/release/native?")
      expect(painted).toContain("11111111 · Ship native delivery")
      click(root.renderer, "apply-git-push")
      expect(applies).toBe(1)
    } finally {
      root.unmount()
    }
  })

  it("does not apply a prepared push while another mutation is active", () => {
    const root = createTestRoot()
    let applies = 0
    try {
      root.render(<div style={{ width: 1200, height: 760 }}><GitPanel git={state()} codeHost={codeHostState({
        mutationBusy: true,
        preparedPush: {
          token: "opaque-one-shot-token",
          remote: {
            remoteName: "origin",
            repository: { provider: "gitHub", host: "github.com", owner: "example-owner", name: "example-project" },
          },
          sourceHead: "1111111111111111111111111111111111111111",
          destinationBranch: "main",
          commits: [{ id: "1111111111111111111111111111111111111111", subject: "One mutation" }],
          truncated: false,
        },
        applyPush: () => { applies += 1 },
      })} /></div>)
      click(root.renderer, "apply-git-push")
      expect(applies).toBe(0)
    } finally {
      root.unmount()
    }
  })
})
