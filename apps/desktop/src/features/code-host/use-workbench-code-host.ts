import { useInfiniteQuery, useQuery } from "@tanstack/react-query"
import type {
  CodeHostAccount,
  CodeHostAccountId,
  CodeHostPullRequestEnsureParams,
  CodeHostPullRequestSummary,
  CodeHostPushPrepareResult,
  CodeHostRemote,
  ProjectId,
  WorkspaceId,
} from "@taugentic/desktop-protocol"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import type { DesktopRuntime } from "../../platform/daemon/desktop-runtime.js"
import {
  codeHostAccountsQuery,
  codeHostApplyPush,
  codeHostConnectAccount,
  codeHostCreatePullRequestComment,
  codeHostDisconnectAccount,
  codeHostEnsurePullRequest,
  codeHostPreparePush,
  codeHostPullRequestActivityQuery,
  codeHostPullRequestChecksQuery,
  codeHostPullRequestDetailQuery,
  codeHostPullRequests,
  codeHostQueryRoot,
  codeHostRepositoryContextQuery,
} from "../../platform/daemon/code-host-query.js"
import { desktopQueryClient } from "../../platform/daemon/query-client.js"

function message(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
}

export function useWorkbenchCodeHost(input: {
  runtime: DesktopRuntime
  projectId?: ProjectId
  workspaceId?: WorkspaceId
  enabled: boolean
  branch?: string
  runActive: boolean
}) {
  const projectId = input.projectId
  const workspaceId = input.workspaceId
  const scoped = input.enabled && Boolean(input.projectId) && Boolean(input.workspaceId)
  const [selectedAccountId, setSelectedAccountId] = useState<CodeHostAccountId>()
  const [selectedRemoteName, setSelectedRemoteName] = useState<string>()
  const [selectedPullRequestNumber, setSelectedPullRequestNumber] = useState<string>()
  const [accountName, setAccountName] = useState("")
  const [accessToken, setAccessToken] = useState("")
  const [accountBusy, setAccountBusy] = useState(false)
  const [mutationBusy, setMutationBusy] = useState(false)
  const accountBusyRef = useRef(false)
  const mutationBusyRef = useRef(false)
  const [error, setError] = useState<string>()
  const [disconnectConfirmation, setDisconnectConfirmation] = useState<CodeHostAccountId>()
  const [pushBranch, setPushBranch] = useState("")
  const [preparedPush, setPreparedPush] = useState<CodeHostPushPrepareResult>()
  const [headRemoteName, setHeadRemoteName] = useState("")
  const [baseRemoteName, setBaseRemoteName] = useState("")
  const [headBranch, setHeadBranch] = useState("")
  const [baseBranch, setBaseBranch] = useState("")
  const [pullRequestTitle, setPullRequestTitle] = useState("")
  const [pullRequestBody, setPullRequestBody] = useState("")
  const [pullRequestDraft, setPullRequestDraft] = useState(false)
  const [commentBody, setCommentBody] = useState("")

  const accountsQuery = useQuery({ ...codeHostAccountsQuery(input.runtime), enabled: input.enabled })
  const repositoryQuery = useQuery({
    ...codeHostRepositoryContextQuery(input.runtime, projectId, workspaceId),
    enabled: scoped,
  })
  const accounts = accountsQuery.data?.accounts ?? []
  const remotes = repositoryQuery.data?.remotes ?? []
  const selectedAccount = accounts.find((account) => account.id === selectedAccountId)
  const selectedRemote = remotes.find((remote) => remote.remoteName === selectedRemoteName)

  useEffect(() => {
    setSelectedAccountId(undefined)
    setSelectedRemoteName(undefined)
    setSelectedPullRequestNumber(undefined)
    setPreparedPush(undefined)
    setPushBranch("")
    setHeadRemoteName("")
    setBaseRemoteName("")
    setHeadBranch("")
    setBaseBranch("")
  }, [projectId, workspaceId])

  useEffect(() => {
    if (selectedAccountId && !accounts.some((account) => account.id === selectedAccountId)) {
      setSelectedAccountId(undefined)
      setSelectedPullRequestNumber(undefined)
      setPreparedPush(undefined)
    }
  }, [accounts, selectedAccountId])

  useEffect(() => {
    if (selectedRemoteName && !remotes.some((remote) => remote.remoteName === selectedRemoteName)) {
      setSelectedRemoteName(undefined)
      setSelectedPullRequestNumber(undefined)
      setPreparedPush(undefined)
    }
  }, [remotes, selectedRemoteName])

  const pullRequestsQuery = useInfiniteQuery({
    queryKey: [...codeHostQueryRoot, "pull-requests", projectId ?? null, workspaceId ?? null, selectedAccountId ?? null, selectedRemote?.repository.host ?? null, selectedRemote?.repository.owner ?? null, selectedRemote?.repository.name ?? null],
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) => {
      if (!projectId || !workspaceId || !selectedAccountId || !selectedRemote) {
        throw new Error("Pull-request list scope is required.")
      }
      return codeHostPullRequests(input.runtime, {
        projectId,
        workspaceId,
        accountId: selectedAccountId,
        repository: selectedRemote.repository,
        cursor: pageParam,
        limit: 50,
      })
    },
    getNextPageParam: (page) => page.nextCursor ?? undefined,
    enabled: scoped && Boolean(selectedAccountId) && Boolean(selectedRemote),
  })
  const pullRequests = useMemo(
    () => pullRequestsQuery.data?.pages.flatMap((page) => page.items) ?? [],
    [pullRequestsQuery.data?.pages],
  )
  const selectedPullRequest = pullRequests.find((pullRequest) => pullRequest.number === selectedPullRequestNumber)

  const pullRequestDetailParams = projectId && workspaceId && selectedAccountId && selectedRemote && selectedPullRequestNumber
    ? { projectId, workspaceId, accountId: selectedAccountId, repository: selectedRemote.repository, number: selectedPullRequestNumber }
    : undefined
  const pullRequestDetailQuery = useQuery({
    ...codeHostPullRequestDetailQuery(input.runtime, pullRequestDetailParams),
    enabled: Boolean(pullRequestDetailParams),
  })
  const detail = pullRequestDetailQuery.data
  const checksParams = projectId && workspaceId && selectedAccountId && selectedRemote && detail
    ? { projectId, workspaceId, accountId: selectedAccountId, repository: selectedRemote.repository, headSha: detail.summary.headSha }
    : undefined
  const checksQuery = useQuery({
    ...codeHostPullRequestChecksQuery(input.runtime, checksParams),
    enabled: Boolean(checksParams),
  })
  const activityParams = projectId && workspaceId && selectedAccountId && selectedRemote && selectedPullRequestNumber
    ? { projectId, workspaceId, accountId: selectedAccountId, repository: selectedRemote.repository, number: selectedPullRequestNumber, limit: 50 }
    : undefined
  const activityQuery = useQuery({
    ...codeHostPullRequestActivityQuery(input.runtime, activityParams),
    enabled: Boolean(activityParams),
  })

  const perform = useCallback(async (operation: () => Promise<void>, fallback: string, account = false) => {
    const busy = account ? accountBusyRef : mutationBusyRef
    if (busy.current) return
    busy.current = true
    account ? setAccountBusy(true) : setMutationBusy(true)
    setError(undefined)
    try {
      await operation()
    } catch (caught) {
      setError(message(caught, fallback))
    } finally {
      busy.current = false
      account ? setAccountBusy(false) : setMutationBusy(false)
    }
  }, [])

  const connectAccount = useCallback(() => {
    if (!accountName.trim() || !accessToken.trim()) return
    const submittedAccessToken = accessToken
    setAccessToken("")
    void perform(async () => {
      const result = await codeHostConnectAccount(input.runtime, {
        provider: "gitHub",
        displayName: accountName,
        host: "github.com",
        accessToken: submittedAccessToken,
      })
      setSelectedAccountId(result.account.id)
      setAccountName("")
    }, "The GitHub account could not be connected.", true)
  }, [accessToken, accountName, input.runtime, perform])

  const disconnectAccount = useCallback((account: CodeHostAccount) => {
    if (disconnectConfirmation !== account.id) {
      setDisconnectConfirmation(account.id)
      return
    }
    void perform(async () => {
      await codeHostDisconnectAccount(input.runtime, { accountId: account.id })
      setDisconnectConfirmation(undefined)
      if (selectedAccountId === account.id) setSelectedAccountId(undefined)
    }, "The GitHub account could not be disconnected.", true)
  }, [disconnectConfirmation, input.runtime, perform, selectedAccountId])

  const preparePush = useCallback(() => {
    if (!input.projectId || !input.workspaceId || !selectedAccountId || !selectedRemoteName || !pushBranch.trim()) return
    void perform(async () => {
      setPreparedPush(await codeHostPreparePush(input.runtime, {
        projectId: input.projectId!,
        workspaceId: input.workspaceId!,
        accountId: selectedAccountId,
        remoteName: selectedRemoteName,
        destinationBranch: pushBranch,
      }))
    }, "The exact push preview could not be prepared.")
  }, [input.projectId, input.runtime, input.workspaceId, perform, pushBranch, selectedAccountId, selectedRemoteName])

  const applyPush = useCallback(() => {
    if (!input.projectId || !input.workspaceId || !preparedPush) return
    const token = preparedPush.token
    setPreparedPush(undefined)
    void perform(async () => {
      await codeHostApplyPush(input.runtime, { token }, input.projectId!, input.workspaceId!)
    }, "The push outcome is unknown. Refresh the repository before trying another operation.")
  }, [input.projectId, input.runtime, input.workspaceId, perform, preparedPush])

  const ensurePullRequest = useCallback(() => {
    if (!input.projectId || !input.workspaceId || !selectedAccountId) return
    const params: CodeHostPullRequestEnsureParams = {
      projectId: input.projectId,
      workspaceId: input.workspaceId,
      accountId: selectedAccountId,
      headRemoteName,
      headBranch,
      baseRemoteName,
      baseBranch,
      title: pullRequestTitle,
      body: pullRequestBody,
      draft: pullRequestDraft,
    }
    void perform(async () => {
      const result = await codeHostEnsurePullRequest(input.runtime, params)
      setSelectedRemoteName(baseRemoteName)
      setSelectedPullRequestNumber(result.pullRequest.number)
      setPullRequestTitle("")
      setPullRequestBody("")
    }, "The pull request outcome is unknown. Refresh before trying another mutation.")
  }, [baseBranch, baseRemoteName, headBranch, headRemoteName, input.projectId, input.runtime, input.workspaceId, perform, pullRequestBody, pullRequestDraft, pullRequestTitle, selectedAccountId])

  const createComment = useCallback(() => {
    if (!input.projectId || !input.workspaceId || !selectedAccountId || !selectedRemote || !selectedPullRequestNumber || !commentBody.trim()) return
    void perform(async () => {
      await codeHostCreatePullRequestComment(input.runtime, {
        projectId: input.projectId!,
        workspaceId: input.workspaceId!,
        accountId: selectedAccountId,
        repository: selectedRemote.repository,
        number: selectedPullRequestNumber,
        body: commentBody,
      })
      setCommentBody("")
    }, "The comment outcome is unknown. Refresh before submitting again.")
  }, [commentBody, input.projectId, input.runtime, input.workspaceId, perform, selectedAccountId, selectedPullRequestNumber, selectedRemote])

  const selectRemote = useCallback((remote: CodeHostRemote) => {
    setSelectedRemoteName(remote.remoteName)
    setSelectedPullRequestNumber(undefined)
    setPreparedPush(undefined)
  }, [])

  const selectAccount = useCallback((account: CodeHostAccount) => {
    setSelectedAccountId(account.id)
    setSelectedPullRequestNumber(undefined)
    setPreparedPush(undefined)
    setDisconnectConfirmation(undefined)
  }, [])

  const refresh = useCallback(() => {
    setError(undefined)
    void desktopQueryClient.invalidateQueries({ queryKey: codeHostQueryRoot })
  }, [])

  const selectPullRequest = useCallback((pullRequest: CodeHostPullRequestSummary) => {
    setSelectedPullRequestNumber(pullRequest.number)
    setCommentBody("")
  }, [])

  return {
    accounts,
    accountsLoading: accountsQuery.isLoading,
    remotes,
    repositoryLoading: repositoryQuery.isLoading,
    selectedAccount,
    selectedRemote,
    selectAccount,
    selectRemote,
    accountName,
    setAccountName,
    accessToken,
    setAccessToken,
    accountBusy,
    connectAccount,
    disconnectAccount,
    disconnectConfirmation,
    pullRequests,
    pullRequestsLoading: pullRequestsQuery.isLoading,
    hasMorePullRequests: Boolean(pullRequestsQuery.hasNextPage),
    loadingMorePullRequests: pullRequestsQuery.isFetchingNextPage,
    loadMorePullRequests: () => void pullRequestsQuery.fetchNextPage(),
    selectedPullRequest,
    selectPullRequest,
    detail,
    detailLoading: pullRequestDetailQuery.isLoading,
    checks: checksQuery.data?.checks ?? [],
    activity: activityQuery.data,
    mutationBusy,
    error: error
      ?? (accountsQuery.isError ? message(accountsQuery.error, "Code-host accounts could not be loaded.") : undefined)
      ?? (repositoryQuery.isError ? message(repositoryQuery.error, "Repository remotes could not be loaded.") : undefined)
      ?? (pullRequestsQuery.isError ? message(pullRequestsQuery.error, "Pull requests could not be loaded.") : undefined)
      ?? (pullRequestDetailQuery.isError ? message(pullRequestDetailQuery.error, "The pull request could not be loaded.") : undefined)
      ?? (checksQuery.isError ? message(checksQuery.error, "Checks could not be loaded.") : undefined)
      ?? (activityQuery.isError ? message(activityQuery.error, "Pull-request activity could not be loaded.") : undefined),
    refresh,
    pushBranch,
    setPushBranch,
    preparedPush,
    cancelPreparedPush: () => setPreparedPush(undefined),
    canPreparePush: scoped && Boolean(selectedAccount) && Boolean(selectedRemote) && Boolean(pushBranch.trim()) && !input.runActive && !mutationBusy,
    preparePush,
    applyPush,
    headRemoteName,
    setHeadRemoteName,
    baseRemoteName,
    setBaseRemoteName,
    headBranch,
    setHeadBranch,
    baseBranch,
    setBaseBranch,
    pullRequestTitle,
    setPullRequestTitle,
    pullRequestBody,
    setPullRequestBody,
    pullRequestDraft,
    setPullRequestDraft,
    canEnsurePullRequest: scoped && Boolean(selectedAccount) && Boolean(headRemoteName) && Boolean(baseRemoteName) && Boolean(headBranch.trim()) && Boolean(baseBranch.trim()) && Boolean(pullRequestTitle.trim()) && !input.runActive && !mutationBusy,
    ensurePullRequest,
    commentBody,
    setCommentBody,
    createComment,
    canCreateComment: Boolean(selectedPullRequest && commentBody.trim()) && !mutationBusy,
    branch: input.branch,
  }
}

export type WorkbenchCodeHostState = ReturnType<typeof useWorkbenchCodeHost>
