import { VirtualList } from "@regenrek/gpuix-react"
import type { GitChangeKind, GitCheckpointPhase, GitFileStatus } from "@taugentic/desktop-protocol"
import { Fragment } from "react"

import { palette } from "../../app/theme.js"
import { Pressable } from "../../ui/pressable.js"
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
      <Pressable testId="refresh-git" name="Refresh Git" onPress={git.refresh} style={actionStyle(true)}><text style={{ fontSize: 10 }}>Refresh</text></Pressable>
    </div>
    {git.error && <div style={{ padding: 10, borderBottomWidth: 1, borderColor: palette.border }}><text testId="git-error" style={{ color: "#F08080", fontSize: 10 }}>{git.error}</text></div>}
    {codeHost.error && <div style={{ padding: 10, borderBottomWidth: 1, borderColor: palette.border }}><text testId="git-delivery-error" style={{ color: "#F08080", fontSize: 10 }}>{codeHost.error}</text></div>}
    <GitPushBar codeHost={codeHost} />
    <div style={{ display: "flex", minHeight: 34, borderBottomWidth: 1, borderColor: palette.border }}>
      {(["unstaged", "staged", "lastTurn"] as const).map((view) => <Fragment key={view}><Pressable
        testId={`git-view-${view}`}
        name={view === "lastTurn" ? "Last turn" : view === "staged" ? "Staged" : "Changes"}
        role="tab"
        selected={git.view === view}
        onPress={() => git.setView(view)}
        style={{ padding: 9, cursor: "pointer", backgroundColor: git.view === view ? palette.panelRaised : palette.canvas }}
      ><text style={{ color: git.view === view ? palette.text : palette.textMuted, fontSize: 10 }}>{view === "lastTurn" ? "Last turn" : view === "staged" ? "Staged" : "Changes"}</text></Pressable></Fragment>)}
    </div>
    <div style={{ display: "flex", minHeight: 0, flexGrow: 1 }}>
      <div style={{ display: "flex", flexDirection: "column", width: 250, minWidth: 180, borderRightWidth: 1, borderColor: palette.border }}>
        <div style={{ display: "flex", alignItems: "center", minHeight: 34, paddingLeft: 10, paddingRight: 8 }}>
          <text style={{ color: palette.textMuted, fontSize: 9 }}>{git.visibleFiles.length} FILES</text><div style={{ flexGrow: 1 }} />
          {git.view === "unstaged" && <Pressable testId="git-stage-selected" name="Stage selected files" disabled={!git.canStage} onPress={git.stageSelected} style={actionStyle(git.canStage)}><text style={{ fontSize: 9 }}>Stage</text></Pressable>}
          {git.view === "staged" && <Pressable testId="git-unstage-selected" name="Unstage selected files" disabled={!git.canUnstage} onPress={git.unstageSelected} style={actionStyle(git.canUnstage)}><text style={{ fontSize: 9 }}>Unstage</text></Pressable>}
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
            return <Fragment key={file.path}><Pressable testId={`git-file-${file.path}`} name={`Select Git file ${file.path}`} role="option" selected={selected} onPress={() => git.togglePath(file.path)} style={{ display: "flex", alignItems: "center", minHeight: 34, paddingLeft: 9, paddingRight: 9, gap: 7, cursor: "pointer", backgroundColor: selected ? palette.panelRaised : palette.canvas }}>
              <text style={{ color: file.unstaged === "untracked" ? palette.warning : palette.accent, width: 16, fontSize: 9 }}>{fileChange(file, git.view)}</text>
              <text style={{ color: palette.text, fontSize: 10, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{file.path}</text>
            </Pressable></Fragment>
          }}
          style={{ flexGrow: 1, minHeight: 0, width: "100%" }}
        />}
      </div>
      <div style={{ display: "flex", flexDirection: "column", minWidth: 0, minHeight: 0, flexGrow: 1 }}>
        {git.preparedRevert && <div testId="git-revert-confirmation" style={{ display: "flex", alignItems: "center", gap: 8, padding: 10, backgroundColor: "#2A2114", borderBottomWidth: 1, borderColor: palette.warning }}>
          <text style={{ color: palette.warning, fontSize: 10, flexGrow: 1 }}>Review this exact patch before restoring {checkpointLabel(git.preparedRevert.checkpoint.phase)}.</text>
          <Pressable testId="cancel-git-revert" name="Cancel checkpoint restore" onPress={git.cancelRevert} style={actionStyle(true)}><text style={{ fontSize: 9 }}>Cancel</text></Pressable>
          <Pressable testId="apply-git-revert" name="Restore checkpoint" disabled={git.busy} onPress={git.applyRevert} style={actionStyle(!git.busy)}><text style={{ fontSize: 9 }}>Restore checkpoint</text></Pressable>
        </div>}
        {git.patchLoading && !git.preparedRevert && <div style={{ padding: 12 }}><text style={{ color: palette.textMuted, fontSize: 10 }}>Loading diff…</text></div>}
        {!git.patchLoading && !git.patch && <div style={{ display: "flex", flexGrow: 1, alignItems: "center", justifyContent: "center" }}><text style={{ color: palette.textMuted, fontSize: 10 }}>No diff in this scope.</text></div>}
        {!!git.patch && <diff testId="git-diff" patch={git.patch} wordDiff scroll style={{ flexGrow: 1, minHeight: 0, width: "100%" }} />}
      </div>
    </div>
    <div style={{ display: "flex", gap: 8, padding: 9, borderTopWidth: 1, borderColor: palette.border }}>
      <input testId="git-commit-message" value={git.commitMessage} placeholder="Commit staged changes" onChange={(event) => git.setCommitMessage(event.value ?? "")} style={{ flexGrow: 1, minWidth: 120, padding: 7, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.panel }} />
      <Pressable testId="git-commit" name="Commit staged changes" disabled={!git.canCommit} onPress={git.commit} style={actionStyle(git.canCommit)}><text style={{ fontSize: 10 }}>Commit</text></Pressable>
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
        {git.checkpoints.slice(-4).reverse().map((checkpoint) => <Fragment key={checkpoint.checkpointId}><div style={{ display: "flex", alignItems: "center", gap: 6 }}><text style={{ color: palette.text, fontSize: 9, flexGrow: 1 }}>{checkpointLabel(checkpoint.phase)} · {checkpoint.runId.slice(-8)}</text><Pressable testId={`prepare-git-revert-${checkpoint.checkpointId}`} name={`Preview ${checkpointLabel(checkpoint.phase)} checkpoint`} disabled={git.busy} onPress={() => git.prepareRevert(checkpoint.checkpointId)} style={actionStyle(!git.busy)}><text style={{ fontSize: 8 }}>Preview</text></Pressable></div></Fragment>)}
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
      {codeHost.remotes.map((remote) => <Fragment key={remote.remoteName}><Pressable testId={`push-remote-${remote.remoteName}`} name={`Push remote ${remote.remoteName}`} role="radio" selected={codeHost.selectedRemote?.remoteName === remote.remoteName} onPress={() => codeHost.selectRemote(remote)} style={actionStyle(true)}><text style={{ color: codeHost.selectedRemote?.remoteName === remote.remoteName ? palette.accent : palette.textMuted, fontSize: 8 }}>{remote.remoteName}</text></Pressable></Fragment>)}
    </div>
    <div style={{ display: "flex", gap: 7 }}>
      <input testId="git-push-branch" value={codeHost.pushBranch} placeholder="Exact destination branch" onChange={(event) => codeHost.setPushBranch(event.value ?? "")} style={{ flexGrow: 1, minWidth: 120, padding: 7, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
      <Pressable testId="prepare-git-push" name="Preview Git push" disabled={!codeHost.canPreparePush} onPress={codeHost.preparePush} style={actionStyle(codeHost.canPreparePush)}><text style={{ fontSize: 9 }}>{codeHost.mutationBusy ? "Checking…" : "Preview push"}</text></Pressable>
    </div>
    {codeHost.preparedPush && <div testId="git-push-confirmation" style={{ display: "flex", flexDirection: "column", gap: 7, padding: 9, borderWidth: 1, borderColor: palette.warning, borderRadius: 7, backgroundColor: "#2A2114" }}>
      <text style={{ color: palette.warning, fontSize: 10 }}>Push {codeHost.preparedPush.commits.length}{codeHost.preparedPush.truncated ? "+" : ""} commit{codeHost.preparedPush.commits.length === 1 ? "" : "s"} to {codeHost.preparedPush.remote.remoteName}/{codeHost.preparedPush.destinationBranch}?</text>
      {codeHost.preparedPush.commits.slice(0, 6).map((commit) => <Fragment key={commit.id}><text style={{ color: palette.textMuted, fontSize: 8, userSelect: "text" }}>{commit.id.slice(0, 8)} · {commit.subject}</text></Fragment>)}
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 7 }}><Pressable testId="cancel-git-push" name="Cancel Git push" onPress={codeHost.cancelPreparedPush} style={actionStyle(true)}><text style={{ fontSize: 9 }}>Cancel</text></Pressable><Pressable testId="apply-git-push" name="Push exact commit" disabled={codeHost.mutationBusy} onPress={codeHost.applyPush} style={actionStyle(!codeHost.mutationBusy)}><text style={{ fontSize: 9 }}>Push exact commit</text></Pressable></div>
    </div>}
  </div>
}
