import { VirtualList } from "@regenrek/gpuix-react"
import type { GitChangeKind, GitCheckpointPhase, GitFileStatus } from "@taugentic/desktop-protocol"
import { Fragment } from "react"

import { palette } from "../../app/theme.js"
import type { WorkbenchCodeHostState } from "../code-host/use-workbench-code-host.js"
import type { GitDiffView, WorkbenchGitState } from "./use-workbench-git.js"

function actionStyle(enabled: boolean) {
  return {
    padding: 7,
    borderRadius: 6,
    backgroundColor: enabled ? palette.accentDim : palette.panelRaised,
    color: enabled ? palette.text : palette.textFaint,
    cursor: enabled ? "pointer" : "default",
  } as const
}

function changeLabel(change?: GitChangeKind | null): string {
  if (!change) return ""
  if (change === "untracked") return "?"
  if (change === "typeChanged") return "T"
  if (change === "unmerged") return "U"
  return change.slice(0, 1).toUpperCase()
}

function checkpointLabel(phase: GitCheckpointPhase): string {
  return phase === "beforeTurn" ? "Before turn" : "After turn"
}

function fileChange(file: GitFileStatus, view: GitDiffView): string {
  if (view === "staged") return changeLabel(file.staged)
  if (view === "unstaged") return changeLabel(file.unstaged)
  return `${changeLabel(file.staged)}${changeLabel(file.unstaged)}` || "·"
}

export function GitPanel({ git, codeHost }: { git: WorkbenchGitState; codeHost: WorkbenchCodeHostState }) {
  const snapshot = git.snapshot
  const branch = snapshot?.branch ?? snapshot?.head?.slice(0, 8) ?? "Unborn repository"
  return <div testId="git-panel" style={{ display: "flex", flexDirection: "column", width: "100%", height: "100%", minHeight: 0, backgroundColor: palette.canvas }}>
    <div style={{ display: "flex", alignItems: "center", minHeight: 42, paddingLeft: 12, paddingRight: 8, gap: 8, borderBottomWidth: 1, borderColor: palette.border }}>
      <text style={{ color: palette.text, fontSize: 12, fontWeight: 650 }}>{branch}</text>
      {snapshot?.upstream && <text style={{ color: palette.textFaint, fontSize: 9 }}>{snapshot.upstream}</text>}
      {!!snapshot?.ahead && <text style={{ color: palette.accent, fontSize: 9 }}>↑{snapshot.ahead}</text>}
      {!!snapshot?.behind && <text style={{ color: palette.warning, fontSize: 9 }}>↓{snapshot.behind}</text>}
      <div style={{ flexGrow: 1 }} />
      <div testId="refresh-git" tabIndex={0} onClick={git.refresh} style={actionStyle(true)}><text style={{ fontSize: 10 }}>Refresh</text></div>
    </div>
    {git.error && <div style={{ padding: 10, borderBottomWidth: 1, borderColor: palette.border }}><text testId="git-error" style={{ color: "#F08080", fontSize: 10 }}>{git.error}</text></div>}
    {codeHost.error && <div style={{ padding: 10, borderBottomWidth: 1, borderColor: palette.border }}><text testId="git-delivery-error" style={{ color: "#F08080", fontSize: 10 }}>{codeHost.error}</text></div>}
    <GitPushBar codeHost={codeHost} />
    <div style={{ display: "flex", minHeight: 34, borderBottomWidth: 1, borderColor: palette.border }}>
      {(["unstaged", "staged", "lastTurn"] as const).map((view) => <Fragment key={view}><div
        testId={`git-view-${view}`}
        tabIndex={0}
        onClick={() => git.setView(view)}
        style={{ padding: 9, cursor: "pointer", backgroundColor: git.view === view ? palette.panelRaised : palette.canvas }}
      ><text style={{ color: git.view === view ? palette.text : palette.textMuted, fontSize: 10 }}>{view === "lastTurn" ? "Last turn" : view === "staged" ? "Staged" : "Changes"}</text></div></Fragment>)}
    </div>
    <div style={{ display: "flex", minHeight: 0, flexGrow: 1 }}>
      <div style={{ display: "flex", flexDirection: "column", width: 250, minWidth: 180, borderRightWidth: 1, borderColor: palette.border }}>
        <div style={{ display: "flex", alignItems: "center", minHeight: 34, paddingLeft: 10, paddingRight: 8 }}>
          <text style={{ color: palette.textMuted, fontSize: 9 }}>{git.visibleFiles.length} FILES</text><div style={{ flexGrow: 1 }} />
          {git.view === "unstaged" && <div testId="git-stage-selected" tabIndex={git.canStage ? 0 : -1} onClick={() => { if (git.canStage) git.stageSelected() }} style={actionStyle(git.canStage)}><text style={{ fontSize: 9 }}>Stage</text></div>}
          {git.view === "staged" && <div testId="git-unstage-selected" tabIndex={git.canUnstage ? 0 : -1} onClick={() => { if (git.canUnstage) git.unstageSelected() }} style={actionStyle(git.canUnstage)}><text style={{ fontSize: 9 }}>Unstage</text></div>}
        </div>
        {git.loading && <div style={{ padding: 12 }}><text style={{ color: palette.textMuted, fontSize: 10 }}>Loading Git status…</text></div>}
        {!git.loading && !git.visibleFiles.length && <div style={{ padding: 12 }}><text style={{ color: palette.textMuted, fontSize: 10 }}>{git.view === "lastTurn" ? "No current file changes." : "No changes in this scope."}</text></div>}
        {!!git.visibleFiles.length && <VirtualList
          itemCount={git.visibleFiles.length}
          estimatedItemHeight={34}
          renderItem={(index) => {
            const file = git.visibleFiles[index]
            if (!file) return null
            const selected = git.selectedPaths.includes(file.path)
            return <Fragment key={file.path}><div testId={`git-file-${file.path}`} tabIndex={0} accessibilitySelected={selected} onClick={() => git.togglePath(file.path)} style={{ display: "flex", alignItems: "center", minHeight: 34, paddingLeft: 9, paddingRight: 9, gap: 7, cursor: "pointer", backgroundColor: selected ? palette.panelRaised : palette.canvas }}>
              <text style={{ color: file.unstaged === "untracked" ? palette.warning : palette.accent, width: 16, fontSize: 9 }}>{fileChange(file, git.view)}</text>
              <text style={{ color: palette.text, fontSize: 10, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{file.path}</text>
            </div></Fragment>
          }}
          style={{ flexGrow: 1, minHeight: 0, width: "100%" }}
        />}
      </div>
      <div style={{ display: "flex", flexDirection: "column", minWidth: 0, minHeight: 0, flexGrow: 1 }}>
        {git.preparedRevert && <div testId="git-revert-confirmation" style={{ display: "flex", alignItems: "center", gap: 8, padding: 10, backgroundColor: "#2A2114", borderBottomWidth: 1, borderColor: palette.warning }}>
          <text style={{ color: palette.warning, fontSize: 10, flexGrow: 1 }}>Review this exact patch before restoring {checkpointLabel(git.preparedRevert.checkpoint.phase)}.</text>
          <div testId="cancel-git-revert" tabIndex={0} onClick={git.cancelRevert} style={actionStyle(true)}><text style={{ fontSize: 9 }}>Cancel</text></div>
          <div testId="apply-git-revert" tabIndex={git.busy ? -1 : 0} onClick={() => { if (!git.busy) git.applyRevert() }} style={actionStyle(!git.busy)}><text style={{ fontSize: 9 }}>Restore checkpoint</text></div>
        </div>}
        {git.patchLoading && !git.preparedRevert && <div style={{ padding: 12 }}><text style={{ color: palette.textMuted, fontSize: 10 }}>Loading diff…</text></div>}
        {!git.patchLoading && !git.patch && <div style={{ display: "flex", flexGrow: 1, alignItems: "center", justifyContent: "center" }}><text style={{ color: palette.textMuted, fontSize: 10 }}>No diff in this scope.</text></div>}
        {!!git.patch && <diff testId="git-diff" patch={git.patch} wordDiff scroll style={{ flexGrow: 1, minHeight: 0, width: "100%" }} />}
      </div>
    </div>
    <div style={{ display: "flex", gap: 8, padding: 9, borderTopWidth: 1, borderColor: palette.border }}>
      <input testId="git-commit-message" value={git.commitMessage} placeholder="Commit staged changes" onChange={(event) => git.setCommitMessage(event.value ?? "")} style={{ flexGrow: 1, minWidth: 120, padding: 7, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.panel }} />
      <div testId="git-commit" tabIndex={git.canCommit ? 0 : -1} onClick={() => { if (git.canCommit) git.commit() }} style={actionStyle(git.canCommit)}><text style={{ fontSize: 10 }}>Commit</text></div>
    </div>
    <div style={{ display: "flex", minHeight: 118, maxHeight: 150, borderTopWidth: 1, borderColor: palette.border }}>
      <div style={{ display: "flex", flexDirection: "column", width: "50%", minWidth: 0, padding: 9, gap: 6 }}>
        <text style={{ color: palette.textMuted, fontSize: 9 }}>WORKTREES</text>
        {!snapshot?.worktrees.length && <text style={{ color: palette.textFaint, fontSize: 9 }}>No worktrees.</text>}
        {snapshot?.worktrees.map((worktree) => <Fragment key={worktree.path}><div style={{ display: "flex", gap: 6 }}><text style={{ color: worktree.current ? palette.accent : palette.textMuted, fontSize: 9 }}>{worktree.current ? "●" : "○"}</text><text style={{ color: palette.text, fontSize: 9, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{worktree.branch ?? worktree.path}</text></div></Fragment>)}
      </div>
      <div style={{ display: "flex", flexDirection: "column", width: "50%", minWidth: 0, padding: 9, gap: 6, borderLeftWidth: 1, borderColor: palette.border }}>
        <text style={{ color: palette.textMuted, fontSize: 9 }}>CHECKPOINTS</text>
        {!git.checkpoints.length && <text style={{ color: palette.textFaint, fontSize: 9 }}>No user-turn checkpoints yet.</text>}
        {git.checkpoints.slice(-4).reverse().map((checkpoint) => <Fragment key={checkpoint.checkpointId}><div style={{ display: "flex", alignItems: "center", gap: 6 }}><text style={{ color: palette.text, fontSize: 9, flexGrow: 1 }}>{checkpointLabel(checkpoint.phase)} · {checkpoint.runId.slice(-8)}</text><div testId={`prepare-git-revert-${checkpoint.checkpointId}`} tabIndex={git.busy ? -1 : 0} onClick={() => { if (!git.busy) git.prepareRevert(checkpoint.checkpointId) }} style={actionStyle(!git.busy)}><text style={{ fontSize: 8 }}>Preview</text></div></div></Fragment>)}
      </div>
    </div>
  </div>
}

function GitPushBar({ codeHost }: { codeHost: WorkbenchCodeHostState }) {
  return <div testId="git-push" style={{ display: "flex", flexDirection: "column", gap: 7, padding: 9, borderBottomWidth: 1, borderColor: palette.border, backgroundColor: palette.panel }}>
    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
      <text style={{ color: palette.textFaint, fontSize: 9 }}>PUSH</text>
      <text style={{ color: codeHost.selectedAccount ? palette.text : palette.warning, fontSize: 9 }}>{codeHost.selectedAccount?.displayName ?? "select account in Pull requests"}</text>
      <div style={{ flexGrow: 1 }} />
      {codeHost.remotes.map((remote) => <Fragment key={remote.remoteName}><div testId={`push-remote-${remote.remoteName}`} tabIndex={0} onClick={() => codeHost.selectRemote(remote)} style={actionStyle(true)}><text style={{ color: codeHost.selectedRemote?.remoteName === remote.remoteName ? palette.accent : palette.textMuted, fontSize: 8 }}>{remote.remoteName}</text></div></Fragment>)}
    </div>
    <div style={{ display: "flex", gap: 7 }}>
      <input testId="git-push-branch" value={codeHost.pushBranch} placeholder="Exact destination branch" onChange={(event) => codeHost.setPushBranch(event.value ?? "")} style={{ flexGrow: 1, minWidth: 120, padding: 7, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
      <div testId="prepare-git-push" tabIndex={codeHost.canPreparePush ? 0 : -1} onClick={() => { if (codeHost.canPreparePush) codeHost.preparePush() }} style={actionStyle(codeHost.canPreparePush)}><text style={{ fontSize: 9 }}>{codeHost.mutationBusy ? "Checking…" : "Preview push"}</text></div>
    </div>
    {codeHost.preparedPush && <div testId="git-push-confirmation" style={{ display: "flex", flexDirection: "column", gap: 7, padding: 9, borderWidth: 1, borderColor: palette.warning, borderRadius: 7, backgroundColor: "#2A2114" }}>
      <text style={{ color: palette.warning, fontSize: 10 }}>Push {codeHost.preparedPush.commits.length}{codeHost.preparedPush.truncated ? "+" : ""} commit{codeHost.preparedPush.commits.length === 1 ? "" : "s"} to {codeHost.preparedPush.remote.remoteName}/{codeHost.preparedPush.destinationBranch}?</text>
      {codeHost.preparedPush.commits.slice(0, 6).map((commit) => <Fragment key={commit.id}><text style={{ color: palette.textMuted, fontSize: 8, userSelect: "text" }}>{commit.id.slice(0, 8)} · {commit.subject}</text></Fragment>)}
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 7 }}><div testId="cancel-git-push" tabIndex={0} onClick={codeHost.cancelPreparedPush} style={actionStyle(true)}><text style={{ fontSize: 9 }}>Cancel</text></div><div testId="apply-git-push" tabIndex={codeHost.mutationBusy ? -1 : 0} onClick={() => { if (!codeHost.mutationBusy) codeHost.applyPush() }} style={actionStyle(!codeHost.mutationBusy)}><text style={{ fontSize: 9 }}>Push exact commit</text></div></div>
    </div>}
  </div>
}
