import { queryOptions } from "@tanstack/react-query"
import type {
  CodeHostAccountConnectParams,
  CodeHostAccountConnectResult,
  CodeHostAccountDisconnectParams,
  CodeHostAccountDisconnectResult,
  CodeHostAccountListResult,
  CodeHostPage,
  CodeHostPullRequestActivityParams,
  CodeHostPullRequestActivityResult,
  CodeHostPullRequestChecksParams,
  CodeHostPullRequestChecksResult,
  CodeHostPullRequestCommentCreateParams,
  CodeHostPullRequestCommentCreateResult,
  CodeHostPullRequestDetail,
  CodeHostPullRequestDetailParams,
  CodeHostPullRequestEnsureParams,
  CodeHostPullRequestEnsureResult,
  CodeHostPullRequestListParams,
  CodeHostPushApplyParams,
  CodeHostPushApplyResult,
  CodeHostPushPrepareParams,
  CodeHostPushPrepareResult,
  CodeHostRepositoryContextParams,
  CodeHostRepositoryContextResult,
  ProjectId,
  WorkspaceId,
} from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"
import { gitQueryRoot } from "./git-query.js"
import { decodeProtocolJson } from "./protocol-json.js"
import { desktopQueryClient } from "./query-client.js"

export const codeHostQueryRoot = ["daemon", "code-host"] as const

export function codeHostAccountsQuery(runtime: DesktopRuntime) {
  return queryOptions({
    queryKey: [...codeHostQueryRoot, "accounts"],
    queryFn: async (): Promise<CodeHostAccountListResult> => decodeProtocolJson(
      await runtime.bridge.codeHostAccounts(),
    ),
  })
}

export function codeHostRepositoryContextQuery(
  runtime: DesktopRuntime,
  projectId?: ProjectId,
  workspaceId?: WorkspaceId,
) {
  return queryOptions({
    queryKey: [...codeHostQueryRoot, "repository", projectId ?? null, workspaceId ?? null],
    queryFn: async (): Promise<CodeHostRepositoryContextResult> => {
      if (!projectId || !workspaceId) throw new Error("Code-host repository scope is required.")
      return decodeProtocolJson(
        await runtime.bridge.codeHostRepositoryContext(JSON.stringify({ projectId, workspaceId })),
      )
    },
  })
}

export async function codeHostConnectAccount(
  runtime: DesktopRuntime,
  params: CodeHostAccountConnectParams,
): Promise<CodeHostAccountConnectResult> {
  const result = decodeProtocolJson<CodeHostAccountConnectResult>(
    await runtime.bridge.connectCodeHostAccount(JSON.stringify(params)),
  )
  await desktopQueryClient.invalidateQueries({ queryKey: [...codeHostQueryRoot, "accounts"] })
  return result
}

export async function codeHostDisconnectAccount(
  runtime: DesktopRuntime,
  params: CodeHostAccountDisconnectParams,
): Promise<CodeHostAccountDisconnectResult> {
  const result = decodeProtocolJson<CodeHostAccountDisconnectResult>(
    await runtime.bridge.disconnectCodeHostAccount(JSON.stringify(params)),
  )
  await desktopQueryClient.invalidateQueries({ queryKey: codeHostQueryRoot })
  return result
}

export async function codeHostPullRequests(
  runtime: DesktopRuntime,
  params: CodeHostPullRequestListParams,
): Promise<CodeHostPage> {
  return decodeProtocolJson(await runtime.bridge.codeHostPullRequests(JSON.stringify(params)))
}

export function codeHostPullRequestDetailQuery(
  runtime: DesktopRuntime,
  params?: CodeHostPullRequestDetailParams,
) {
  return queryOptions({
    queryKey: [...codeHostQueryRoot, "pull-request", params?.accountId ?? null, params?.repository.host ?? null, params?.repository.owner ?? null, params?.repository.name ?? null, params?.number ?? null],
    queryFn: async (): Promise<CodeHostPullRequestDetail> => {
      if (!params) throw new Error("Pull-request detail scope is required.")
      return decodeProtocolJson(
        await runtime.bridge.codeHostPullRequestDetail(JSON.stringify(params)),
      )
    },
  })
}

export function codeHostPullRequestChecksQuery(
  runtime: DesktopRuntime,
  params?: CodeHostPullRequestChecksParams,
) {
  return queryOptions({
    queryKey: [...codeHostQueryRoot, "checks", params?.accountId ?? null, params?.repository.host ?? null, params?.repository.owner ?? null, params?.repository.name ?? null, params?.headSha ?? null],
    queryFn: async (): Promise<CodeHostPullRequestChecksResult> => {
      if (!params) throw new Error("Pull-request check scope is required.")
      return decodeProtocolJson(
        await runtime.bridge.codeHostPullRequestChecks(JSON.stringify(params)),
      )
    },
  })
}

export function codeHostPullRequestActivityQuery(
  runtime: DesktopRuntime,
  params?: CodeHostPullRequestActivityParams,
) {
  return queryOptions({
    queryKey: [...codeHostQueryRoot, "activity", params?.accountId ?? null, params?.repository.host ?? null, params?.repository.owner ?? null, params?.repository.name ?? null, params?.number ?? null, params?.cursor ?? null],
    queryFn: async (): Promise<CodeHostPullRequestActivityResult> => {
      if (!params) throw new Error("Pull-request activity scope is required.")
      return decodeProtocolJson(
        await runtime.bridge.codeHostPullRequestActivity(JSON.stringify(params)),
      )
    },
  })
}

export async function codeHostEnsurePullRequest(
  runtime: DesktopRuntime,
  params: CodeHostPullRequestEnsureParams,
): Promise<CodeHostPullRequestEnsureResult> {
  const result = decodeProtocolJson<CodeHostPullRequestEnsureResult>(
    await runtime.bridge.ensureCodeHostPullRequest(JSON.stringify(params)),
  )
  await desktopQueryClient.invalidateQueries({ queryKey: codeHostQueryRoot })
  return result
}

export async function codeHostCreatePullRequestComment(
  runtime: DesktopRuntime,
  params: CodeHostPullRequestCommentCreateParams,
): Promise<CodeHostPullRequestCommentCreateResult> {
  const result = decodeProtocolJson<CodeHostPullRequestCommentCreateResult>(
    await runtime.bridge.createCodeHostPullRequestComment(JSON.stringify(params)),
  )
  await desktopQueryClient.invalidateQueries({ queryKey: codeHostQueryRoot })
  return result
}

export async function codeHostPreparePush(
  runtime: DesktopRuntime,
  params: CodeHostPushPrepareParams,
): Promise<CodeHostPushPrepareResult> {
  return decodeProtocolJson(await runtime.bridge.prepareCodeHostPush(JSON.stringify(params)))
}

export async function codeHostApplyPush(
  runtime: DesktopRuntime,
  params: CodeHostPushApplyParams,
  projectId: ProjectId,
  workspaceId: WorkspaceId,
): Promise<CodeHostPushApplyResult> {
  const result = decodeProtocolJson<CodeHostPushApplyResult>(
    await runtime.bridge.applyCodeHostPush(JSON.stringify(params)),
  )
  await Promise.all([
    desktopQueryClient.invalidateQueries({ queryKey: codeHostQueryRoot }),
    desktopQueryClient.invalidateQueries({ queryKey: [...gitQueryRoot, projectId, workspaceId] }),
  ])
  return result
}
