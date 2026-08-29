import { createTestRoot } from "@regenrek/gpuix-react/testing"
import type {
  CodeHostAccount,
  CodeHostPullRequestDetail,
  CodeHostPullRequestSummary,
  CodeHostRemote,
} from "@taugentic/desktop-protocol"
import { describe, expect, it } from "bun:test"

import { PullRequestsPanel } from "../src/features/code-host/pull-requests-panel.js"
import { codeHostState } from "./support/code-host.js"

const account: CodeHostAccount = {
  id: "account-work",
  provider: "gitHub",
  displayName: "Work account",
  accountLogin: "example-user",
  host: "github.com",
}

const repository = {
  provider: "gitHub" as const,
  host: "github.com",
  owner: "example-owner",
  name: "example-project",
}

const remote: CodeHostRemote = {
  remoteName: "origin",
  repository,
}

const pullRequest: CodeHostPullRequestSummary = {
  id: "pull-request-17",
  number: "17",
  title: "Ship the native code-host surface",
  state: "open",
  draft: false,
  authorLogin: "example-author",
  headRepository: repository,
  headBranch: "feature/native-delivery",
  headSha: "1111111111111111111111111111111111111111",
  baseRepository: repository,
  baseBranch: "main",
  webUrl: "https://github.com/example-owner/example-project/pull/17",
  updatedAt: "2026-08-27T10:00:00Z",
}

const detail: CodeHostPullRequestDetail = {
  summary: pullRequest,
  body: "A bounded native delivery change.",
  mergeable: true,
  additions: 12,
  deletions: 3,
  changedFiles: 4,
}

function click(renderer: ReturnType<typeof createTestRoot>["renderer"], testId: string) {
  const element = renderer.findByTestId(testId)!
  const bounds = renderer.getElementBounds(element.id)
  expect(bounds, `${testId} must have native bounds`).not.toBeNull()
  const [x = 0, y = 0, width = 0, height = 0] = bounds ?? []
  renderer.nativeSimulateClick(x + width / 2, y + height / 2)
}

describe("M6 native code-host workbench", () => {
  it("never paints an imported access token", () => {
    const root = createTestRoot()
    try {
      root.render(<div style={{ width: 1200, height: 760 }}><PullRequestsPanel
        codeHost={codeHostState({ accessToken: "secret-marker-that-must-not-paint" })}
        openUrl={() => {}}
      /></div>)
      expect(root.renderer.getPaintedText()).not.toContain("secret-marker-that-must-not-paint")
      expect(root.renderer.getPaintedText()).toContain("Token ready · characters hidden")
      expect(root.renderer.getAutomationTree()).not.toContain("secret-marker-that-must-not-paint")
      expect(JSON.stringify(root.renderer.toJSON())).not.toContain("secret-marker-that-must-not-paint")
    } finally {
      root.unmount()
    }
  })

  it("does not infer an account or repository and exposes explicit selectors", () => {
    const root = createTestRoot()
    let selectedAccounts = 0
    try {
      root.render(<div style={{ width: 1200, height: 760 }}><PullRequestsPanel
        codeHost={codeHostState({
          accounts: [account],
          remotes: [remote],
          selectAccount: () => { selectedAccounts += 1 },
        })}
        openUrl={() => {}}
      /></div>)
      expect(root.renderer.getPaintedText()).toContain("Select a connected account. No account is chosen implicitly.")
      click(root.renderer, "select-code-host-account-account-work")
      expect(selectedAccounts).toBe(1)
      expect(root.renderer.findByTestId("select-code-host-remote-origin")).toBeUndefined()
    } finally {
      root.unmount()
    }
  })

  it("renders typed pull-request detail, checks, reviews, comments, and timeline", () => {
    const root = createTestRoot()
    const opened: string[] = []
    let comments = 0
    try {
      root.render(<div style={{ width: 1100, height: 760 }}><PullRequestsPanel
        codeHost={codeHostState({
          accounts: [account],
          remotes: [remote],
          selectedAccount: account,
          selectedRemote: remote,
          pullRequests: [pullRequest],
          selectedPullRequest: pullRequest,
          detail,
          checks: [{ id: "check-1", name: "Native tests", status: "completed", conclusion: "success" }],
          activity: {
            comments: [{ id: "comment-1", kind: "conversation", authorLogin: "reviewer", body: "Looks good.", webUrl: pullRequest.webUrl, createdAt: "2026-08-27T10:01:00Z" }],
            reviews: [{ id: "review-1", authorLogin: "maintainer", state: "approved", body: "Approved.", webUrl: pullRequest.webUrl, submittedAt: "2026-08-27T10:02:00Z" }],
            timeline: [{ id: "event-1", kind: "merged-base", actorLogin: "system", summary: "Base branch updated.", createdAt: "2026-08-27T10:03:00Z" }],
          },
          commentBody: "Thank you.",
          canCreateComment: true,
          createComment: () => { comments += 1 },
        })}
        openUrl={(url) => opened.push(url)}
      /></div>)
      const painted = root.renderer.getPaintedText()
      expect(painted).toContain("Native tests")
      expect(painted).toContain("Approved.")
      expect(painted).toContain("Looks good.")
      expect(painted).toContain("Base branch updated.")
      click(root.renderer, "open-pull-request")
      const detailElement = root.renderer.findByTestId("pull-request-detail")!
      root.renderer.scrollTo(detailElement.id, 0, -1000)
      click(root.renderer, "create-pull-request-comment")
      expect(opened).toEqual([pullRequest.webUrl])
      expect(comments).toBe(1)
    } finally {
      root.unmount()
    }
  })

  it("requires explicit head/base inputs and blocks duplicate mutation controls", () => {
    const root = createTestRoot()
    let ensureCalls = 0
    let commentCalls = 0
    try {
      root.render(<div style={{ width: 1100, height: 760 }}><PullRequestsPanel
        codeHost={codeHostState({
          accounts: [account],
          remotes: [remote],
          selectedAccount: account,
          selectedRemote: remote,
          pullRequests: [pullRequest],
          selectedPullRequest: pullRequest,
          detail,
          mutationBusy: true,
          canEnsurePullRequest: false,
          canCreateComment: false,
          ensurePullRequest: () => { ensureCalls += 1 },
          createComment: () => { commentCalls += 1 },
        })}
        openUrl={() => {}}
      /></div>)
      click(root.renderer, "toggle-pull-request-create")
      expect(root.renderer.getPaintedText().join("")).toContain("Choose the head and base explicitly. Taugentic matches the exact repository and branch pair before creating anything.")
      click(root.renderer, "ensure-pull-request")
      click(root.renderer, "create-pull-request-comment")
      expect(ensureCalls).toBe(0)
      expect(commentCalls).toBe(0)
    } finally {
      root.unmount()
    }
  })
})
