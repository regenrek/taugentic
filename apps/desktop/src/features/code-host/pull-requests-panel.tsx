import { VirtualList } from "@regenrek/gpuix-react"
import type { CodeHostCheck, CodeHostPullRequestSummary } from "@taugentic/desktop-protocol"
import { Fragment, useState } from "react"

import { palette } from "../../app/theme.js"
import { Pressable } from "../../ui/pressable.js"
import type { WorkbenchCodeHostState } from "./use-workbench-code-host.js"

function button(enabled = true, selected = false) {
  return {
    padding: 7,
    borderRadius: 6,
    backgroundColor: selected ? palette.accentDim : palette.panelRaised,
    color: enabled ? palette.text : palette.textFaint,
    cursor: enabled ? "pointer" : "default",
  } as const
}

function checkMark(check: CodeHostCheck): string {
  if (check.status === "inProgress") return "◐"
  if (check.status === "queued") return "○"
  if (check.conclusion === "success") return "✓"
  if (check.conclusion === "neutral" || check.conclusion === "skipped") return "–"
  return "×"
}

function checkColor(check: CodeHostCheck): string {
  if (check.status !== "completed") return palette.warning
  return check.conclusion === "success" ? palette.accent : check.conclusion === "neutral" || check.conclusion === "skipped" ? palette.textMuted : "#f08080"
}

export function PullRequestsPanel({ codeHost, openUrl }: { codeHost: WorkbenchCodeHostState; openUrl(url: string): void }) {
  const [showConnection, setShowConnection] = useState(false)
  const [showCreation, setShowCreation] = useState(false)
  return <div testId="pull-requests-panel" style={{ display: "flex", flexDirection: "column", width: "100%", height: "100%", minHeight: 0, backgroundColor: palette.canvas }}>
    <div style={{ display: "flex", alignItems: "center", gap: 7, minHeight: 42, paddingLeft: 10, paddingRight: 8, borderBottomWidth: 1, borderColor: palette.border }}>
      <text style={{ color: palette.text, fontSize: 12, fontWeight: 650 }}>Pull requests</text>
      <div style={{ flexGrow: 1 }} />
      <Pressable testId="refresh-code-host" name="Refresh pull requests" onPress={codeHost.refresh} style={button()}><text style={{ fontSize: 9 }}>Refresh</text></Pressable>
      <Pressable testId="toggle-code-host-connection" name="Accounts" expanded={showConnection} onPress={() => setShowConnection((current) => !current)} style={button()}><text style={{ fontSize: 9 }}>Accounts</text></Pressable>
      <Pressable testId="toggle-pull-request-create" name="New pull request" disabled={!codeHost.selectedAccount} expanded={showCreation} onPress={() => setShowCreation((current) => !current)} style={button(Boolean(codeHost.selectedAccount))}><text style={{ fontSize: 9 }}>New PR</text></Pressable>
    </div>
    {codeHost.error && <div style={{ padding: 9, borderBottomWidth: 1, borderColor: palette.border }}><text testId="code-host-error" style={{ color: "#f08080", fontSize: 10, userSelect: "text" }}>{codeHost.error}</text></div>}
    {(showConnection || !codeHost.accounts.length) && <AccountConnections codeHost={codeHost} />}
    {!!codeHost.accounts.length && <div style={{ display: "flex", flexWrap: "wrap", gap: 6, padding: 8, borderBottomWidth: 1, borderColor: palette.border }}>
      <text style={{ color: palette.textFaint, fontSize: 9, padding: 7 }}>ACCOUNT</text>
      {codeHost.accounts.map((account) => <Fragment key={account.id}><Pressable testId={`select-code-host-account-${account.id}`} name={`Account ${account.displayName}`} role="radio" selected={codeHost.selectedAccount?.id === account.id} onPress={() => codeHost.selectAccount(account)} style={button(true, codeHost.selectedAccount?.id === account.id)}><text style={{ fontSize: 9 }}>{account.displayName}</text></Pressable></Fragment>)}
    </div>}
    {!!codeHost.selectedAccount && <div style={{ display: "flex", flexWrap: "wrap", gap: 6, padding: 8, borderBottomWidth: 1, borderColor: palette.border }}>
      <text style={{ color: palette.textFaint, fontSize: 9, padding: 7 }}>REPOSITORY</text>
      {codeHost.repositoryLoading && <text style={{ color: palette.textMuted, fontSize: 9, padding: 7 }}>Loading remotes…</text>}
      {!codeHost.repositoryLoading && !codeHost.remotes.length && <text style={{ color: palette.textMuted, fontSize: 9, padding: 7 }}>No supported GitHub remotes.</text>}
      {codeHost.remotes.map((remote) => <Fragment key={remote.remoteName}><Pressable testId={`select-code-host-remote-${remote.remoteName}`} name={`Repository ${remote.remoteName} ${remote.repository.owner}/${remote.repository.name}`} role="radio" selected={codeHost.selectedRemote?.remoteName === remote.remoteName} onPress={() => codeHost.selectRemote(remote)} style={button(true, codeHost.selectedRemote?.remoteName === remote.remoteName)}><text style={{ fontSize: 9 }}>{remote.remoteName} · {remote.repository.owner}/{remote.repository.name}</text></Pressable></Fragment>)}
    </div>}
    {showCreation && <PullRequestCreation codeHost={codeHost} onClose={() => setShowCreation(false)} />}
    {!codeHost.selectedAccount && <Empty message="Select a connected account. No account is chosen implicitly." />}
    {codeHost.selectedAccount && !codeHost.selectedRemote && <Empty message="Select the exact repository remote to inspect." />}
    {codeHost.selectedAccount && codeHost.selectedRemote && <div style={{ display: "flex", flexGrow: 1, minHeight: 0 }}>
      <PullRequestList codeHost={codeHost} />
      <PullRequestDetail codeHost={codeHost} openUrl={openUrl} />
    </div>}
  </div>
}

function Empty({ message }: { message: string }) {
  return <div style={{ display: "flex", alignItems: "center", justifyContent: "center", flexGrow: 1, padding: 20 }}><text style={{ color: palette.textMuted, fontSize: 11, textAlign: "center" }}>{message}</text></div>
}

function AccountConnections({ codeHost }: { codeHost: WorkbenchCodeHostState }) {
  return <div testId="code-host-accounts" style={{ display: "flex", flexDirection: "column", gap: 8, padding: 10, backgroundColor: palette.panel, borderBottomWidth: 1, borderColor: palette.border }}>
    <text style={{ color: palette.text, fontSize: 11, fontWeight: 600 }}>GitHub connections</text>
    <text style={{ color: palette.textMuted, fontSize: 9 }}>Each local profile has its own OS-protected token. Tokens are never stored with project data.</text>
    {codeHost.accounts.map((account) => <Fragment key={account.id}><div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <div style={{ display: "flex", flexDirection: "column", flexGrow: 1 }}><text style={{ color: palette.text, fontSize: 10 }}>{account.displayName}</text><text style={{ color: palette.textFaint, fontSize: 9 }}>{account.accountLogin} · {account.host}</text></div>
      <Pressable testId={`disconnect-code-host-account-${account.id}`} name={`${codeHost.disconnectConfirmation === account.id ? "Confirm disconnect" : "Disconnect"} ${account.displayName}`} disabled={codeHost.accountBusy} onPress={() => codeHost.disconnectAccount(account)} style={button(!codeHost.accountBusy, codeHost.disconnectConfirmation === account.id)}><text style={{ fontSize: 9 }}>{codeHost.disconnectConfirmation === account.id ? "Confirm disconnect" : "Disconnect"}</text></Pressable>
    </div></Fragment>)}
    <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
      <input testId="code-host-account-name" value={codeHost.accountName} placeholder="Local profile name" onChange={(event) => codeHost.setAccountName(event.value ?? "")} style={{ flexGrow: 1, minWidth: 150, padding: 7, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
      <div style={{ display: "flex", flexDirection: "column", flexGrow: 1, minWidth: 180 }}>
        <input testId="code-host-access-token" secure value={codeHost.accessToken} placeholder="Paste access token" onChange={(event) => codeHost.setAccessToken(event.value ?? "")} style={{ padding: 7, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
        {!!codeHost.accessToken && <text testId="code-host-token-ready" style={{ color: palette.accent, fontSize: 8 }}>Token ready · characters hidden</text>}
      </div>
      <Pressable testId="connect-code-host-account" name="Connect code host account" disabled={!codeHost.accountName.trim() || !codeHost.accessToken.trim() || codeHost.accountBusy} onPress={codeHost.connectAccount} style={button(Boolean(codeHost.accountName.trim() && codeHost.accessToken.trim() && !codeHost.accountBusy))}><text style={{ fontSize: 9 }}>{codeHost.accountBusy ? "Connecting…" : "Connect"}</text></Pressable>
    </div>
  </div>
}

function PullRequestCreation({ codeHost, onClose }: { codeHost: WorkbenchCodeHostState; onClose(): void }) {
  return <div testId="pull-request-create" style={{ display: "flex", flexDirection: "column", gap: 7, padding: 10, backgroundColor: palette.panel, borderBottomWidth: 1, borderColor: palette.border }}>
    <div style={{ display: "flex", gap: 7, alignItems: "center" }}><text style={{ color: palette.text, fontSize: 11, fontWeight: 600, flexGrow: 1 }}>Open pull request</text><Pressable name="Close pull request creation" onPress={onClose} style={button()}><text style={{ fontSize: 9 }}>Close</text></Pressable></div>
    <text style={{ color: palette.textMuted, fontSize: 9 }}>Choose the head and base explicitly. Taugentic matches the exact repository and branch pair before creating anything.</text>
    <div style={{ display: "flex", gap: 8 }}>
      <RemoteBranch label="HEAD" remotes={codeHost.remotes} remoteName={codeHost.headRemoteName} setRemoteName={codeHost.setHeadRemoteName} branch={codeHost.headBranch} setBranch={codeHost.setHeadBranch} placeholder={codeHost.branch ?? "feature branch"} />
      <RemoteBranch label="BASE" remotes={codeHost.remotes} remoteName={codeHost.baseRemoteName} setRemoteName={codeHost.setBaseRemoteName} branch={codeHost.baseBranch} setBranch={codeHost.setBaseBranch} placeholder="main" />
    </div>
    <input testId="pull-request-title" value={codeHost.pullRequestTitle} placeholder="Pull request title" onChange={(event) => codeHost.setPullRequestTitle(event.value ?? "")} style={{ padding: 7, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
    <textarea testId="pull-request-body" value={codeHost.pullRequestBody} placeholder="Description" minRows={2} maxRows={6} onChange={(event) => codeHost.setPullRequestBody(event.value ?? "")} style={{ padding: 7, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.canvas }} />
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}><Pressable testId="pull-request-draft" name="Draft pull request" role="checkbox" checked={codeHost.pullRequestDraft} onPress={() => codeHost.setPullRequestDraft(!codeHost.pullRequestDraft)} style={button(true, codeHost.pullRequestDraft)}><text style={{ fontSize: 9 }}>{codeHost.pullRequestDraft ? "Draft ✓" : "Draft"}</text></Pressable><div style={{ flexGrow: 1 }} /><Pressable testId="ensure-pull-request" name="Find or create pull request" disabled={!codeHost.canEnsurePullRequest} onPress={codeHost.ensurePullRequest} style={button(codeHost.canEnsurePullRequest)}><text style={{ fontSize: 9 }}>{codeHost.mutationBusy ? "Working…" : "Find or create"}</text></Pressable></div>
  </div>
}

function RemoteBranch(props: { label: string; remotes: WorkbenchCodeHostState["remotes"]; remoteName: string; setRemoteName(value: string): void; branch: string; setBranch(value: string): void; placeholder: string }) {
  return <div style={{ display: "flex", flexDirection: "column", width: "50%", gap: 5 }}><text style={{ color: palette.textFaint, fontSize: 8 }}>{props.label}</text><div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>{props.remotes.map((remote) => <Fragment key={remote.remoteName}><Pressable name={`${props.label} remote ${remote.remoteName}`} role="radio" selected={props.remoteName === remote.remoteName} onPress={() => props.setRemoteName(remote.remoteName)} style={button(true, props.remoteName === remote.remoteName)}><text style={{ fontSize: 8 }}>{remote.remoteName}</text></Pressable></Fragment>)}</div><input value={props.branch} placeholder={props.placeholder} onChange={(event) => props.setBranch(event.value ?? "")} style={{ padding: 6, borderWidth: 1, borderColor: palette.border, borderRadius: 5, color: palette.text, backgroundColor: palette.canvas }} /></div>
}

function PullRequestList({ codeHost }: { codeHost: WorkbenchCodeHostState }) {
  return <div style={{ display: "flex", flexDirection: "column", width: 270, minWidth: 210, minHeight: 0, borderRightWidth: 1, borderColor: palette.border }}>
    {codeHost.pullRequestsLoading && <div style={{ padding: 12 }}><text style={{ color: palette.textMuted, fontSize: 10 }}>Loading pull requests…</text></div>}
    {!codeHost.pullRequestsLoading && !codeHost.pullRequests.length && <div style={{ padding: 12 }}><text testId="pull-request-empty" style={{ color: palette.textMuted, fontSize: 10 }}>No open pull requests.</text></div>}
    {!!codeHost.pullRequests.length && <VirtualList itemCount={codeHost.pullRequests.length} estimatedItemHeight={64} renderItem={(index) => {
      const pullRequest = codeHost.pullRequests[index]
      if (!pullRequest) return null
      return <PullRequestRow key={pullRequest.id} pullRequest={pullRequest} selected={codeHost.selectedPullRequest?.id === pullRequest.id} onClick={() => codeHost.selectPullRequest(pullRequest)} />
    }} style={{ flexGrow: 1, minHeight: 0, width: "100%" }} />}
    {codeHost.hasMorePullRequests && <Pressable testId="load-more-pull-requests" name="Load more pull requests" disabled={codeHost.loadingMorePullRequests} onPress={codeHost.loadMorePullRequests} style={{ padding: 9, cursor: "pointer", borderTopWidth: 1, borderColor: palette.border }}><text style={{ color: palette.textMuted, fontSize: 9 }}>{codeHost.loadingMorePullRequests ? "Loading…" : "Load more"}</text></Pressable>}
  </div>
}

function PullRequestRow({ pullRequest, selected, onClick }: { pullRequest: CodeHostPullRequestSummary; selected: boolean; onClick(): void }) {
  return <Pressable testId={`pull-request-${pullRequest.number}`} name={`Pull request ${pullRequest.number} ${pullRequest.title}`} role="option" selected={selected} onPress={onClick} style={{ display: "flex", flexDirection: "column", gap: 5, minHeight: 62, padding: 10, cursor: "pointer", backgroundColor: selected ? palette.panelRaised : palette.canvas }}><div style={{ display: "flex", gap: 6 }}><text style={{ color: palette.accent, fontSize: 9 }}>#{pullRequest.number}</text>{pullRequest.draft && <text style={{ color: palette.warning, fontSize: 9 }}>DRAFT</text>}<text style={{ color: palette.textFaint, fontSize: 9 }}>{pullRequest.authorLogin}</text></div><text style={{ color: palette.text, fontSize: 10, fontWeight: 600 }}>{pullRequest.title}</text><text style={{ color: palette.textMuted, fontSize: 8 }}>{pullRequest.headBranch} → {pullRequest.baseBranch}</text></Pressable>
}

function PullRequestDetail({ codeHost, openUrl }: { codeHost: WorkbenchCodeHostState; openUrl(url: string): void }) {
  const detail = codeHost.detail
  if (!codeHost.selectedPullRequest) return <Empty message="Select a pull request to inspect details, checks, reviews, and activity." />
  if (codeHost.detailLoading || !detail) return <Empty message="Loading pull request…" />
  return <div testId="pull-request-detail" style={{ display: "flex", flexDirection: "column", flexGrow: 1, minWidth: 0, minHeight: 0, overflow: "scroll", padding: 14, gap: 12 }}>
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}><text style={{ color: palette.text, fontSize: 16, fontWeight: 650, flexGrow: 1 }}>{detail.summary.title}</text><Pressable testId="open-pull-request" name="Open pull request on GitHub" onPress={() => openUrl(detail.summary.webUrl)} style={button()}><text style={{ fontSize: 9 }}>Open on GitHub</text></Pressable></div>
    <text style={{ color: palette.textMuted, fontSize: 9 }}>{detail.summary.headRepository.owner}/{detail.summary.headRepository.name}:{detail.summary.headBranch} → {detail.summary.baseRepository.owner}/{detail.summary.baseRepository.name}:{detail.summary.baseBranch}</text>
    <text style={{ color: palette.textFaint, fontSize: 9 }}>+{detail.additions} −{detail.deletions} · {detail.changedFiles} files</text>
    {!!detail.body && <markdown source={detail.body} />}
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}><text style={{ color: palette.textMuted, fontSize: 9 }}>CHECKS</text>{!codeHost.checks.length && <text style={{ color: palette.textFaint, fontSize: 9 }}>No checks reported.</text>}{codeHost.checks.map((check) => <Fragment key={check.id}><div style={{ display: "flex", gap: 7 }}><text style={{ color: checkColor(check), fontSize: 10 }}>{checkMark(check)}</text><text style={{ color: palette.text, fontSize: 9 }}>{check.name}</text><text style={{ color: palette.textFaint, fontSize: 9 }}>{check.conclusion ?? check.status}</text></div></Fragment>)}</div>
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}><text style={{ color: palette.textMuted, fontSize: 9 }}>REVIEWS & ACTIVITY</text>{codeHost.activity?.reviews.map((review) => <Fragment key={review.id}><ActivityCard author={review.authorLogin} label={`review · ${review.state}`} body={review.body} /></Fragment>)}{codeHost.activity?.comments.map((comment) => <Fragment key={comment.id}><ActivityCard author={comment.authorLogin} label={comment.kind} body={comment.body} /></Fragment>)}{codeHost.activity?.timeline.map((item) => <Fragment key={item.id}><ActivityCard author={item.actorLogin} label={item.kind} body={item.summary} /></Fragment>)}</div>
    <div style={{ display: "flex", gap: 7, alignItems: "flex-end" }}><textarea testId="pull-request-comment" value={codeHost.commentBody} placeholder="Add a comment" minRows={2} maxRows={6} onChange={(event) => codeHost.setCommentBody(event.value ?? "")} style={{ flexGrow: 1, padding: 7, borderWidth: 1, borderColor: palette.border, borderRadius: 6, color: palette.text, backgroundColor: palette.panel }} /><Pressable testId="create-pull-request-comment" name="Create pull request comment" disabled={!codeHost.canCreateComment} onPress={codeHost.createComment} style={button(codeHost.canCreateComment)}><text style={{ fontSize: 9 }}>{codeHost.mutationBusy ? "Sending…" : "Comment"}</text></Pressable></div>
  </div>
}

function ActivityCard({ author, label, body }: { author: string; label: string; body: string }) {
  return <div style={{ display: "flex", flexDirection: "column", gap: 5, padding: 9, borderWidth: 1, borderColor: palette.border, borderRadius: 7, backgroundColor: palette.panel }}><div style={{ display: "flex", gap: 7 }}><text style={{ color: palette.text, fontSize: 9, fontWeight: 600 }}>{author}</text><text style={{ color: palette.textFaint, fontSize: 8 }}>{label}</text></div>{body && <markdown source={body} />}</div>
}
