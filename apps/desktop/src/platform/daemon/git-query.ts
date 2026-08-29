import { queryOptions } from "@tanstack/react-query"
import type {
  GitCheckpointApplyRevertParams,
  GitCheckpointListResult,
  GitCheckpointPrepareRevertParams,
  GitCheckpointPrepareRevertResult,
  GitCommitParams,
  GitDiffResult,
  GitDiffScope,
  GitMutationResult,
  GitPathsMutationParams,
  GitRepositorySnapshotResult,
  ProjectId,
  WorkspaceId,
} from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"
import { decodeProtocolJson } from "./protocol-json.js"
import { desktopQueryClient } from "./query-client.js"
import { workspaceFilesQueryRoot } from "./workspace-files-query.js"

export const gitQueryRoot = ["daemon", "git"] as const

function gitScopeKey(projectId: ProjectId, workspaceId: WorkspaceId) {
  return [...gitQueryRoot, projectId, workspaceId] as const
}

export function gitSnapshotQuery(runtime: DesktopRuntime, projectId: ProjectId, workspaceId: WorkspaceId) {
  return queryOptions({
    queryKey: [...gitScopeKey(projectId, workspaceId), "snapshot"],
    queryFn: async (): Promise<GitRepositorySnapshotResult> => decodeProtocolJson(
      await runtime.bridge.gitSnapshot(JSON.stringify({ projectId, workspaceId })),
    ),
  })
}

export function gitDiffQuery(runtime: DesktopRuntime, projectId: ProjectId, workspaceId: WorkspaceId, scope: GitDiffScope) {
  const scopeKey = scope.kind === "checkpoint" ? `${scope.kind}:${scope.checkpointId}` : scope.kind
  return queryOptions({
    queryKey: [...gitScopeKey(projectId, workspaceId), "diff", scopeKey],
    queryFn: async (): Promise<GitDiffResult> => decodeProtocolJson(
      await runtime.bridge.gitDiff(JSON.stringify({ projectId, workspaceId, scope })),
    ),
  })
}

export function gitCheckpointsQuery(runtime: DesktopRuntime, projectId: ProjectId, workspaceId: WorkspaceId) {
  return queryOptions({
    queryKey: [...gitScopeKey(projectId, workspaceId), "checkpoints"],
    queryFn: async (): Promise<GitCheckpointListResult> => decodeProtocolJson(
      await runtime.bridge.gitCheckpointList(JSON.stringify({ projectId, workspaceId })),
    ),
  })
}

async function invalidateGitScope(projectId: ProjectId, workspaceId: WorkspaceId): Promise<void> {
  await desktopQueryClient.invalidateQueries({ queryKey: gitScopeKey(projectId, workspaceId) })
}

async function invalidateWorkspaceAfterGitMutation(projectId: ProjectId, workspaceId: WorkspaceId): Promise<void> {
  await Promise.all([
    invalidateGitScope(projectId, workspaceId),
    desktopQueryClient.invalidateQueries({ queryKey: workspaceFilesQueryRoot }),
  ])
}

export async function gitStage(runtime: DesktopRuntime, params: GitPathsMutationParams): Promise<GitMutationResult> {
  const result = decodeProtocolJson<GitMutationResult>(await runtime.bridge.gitStage(JSON.stringify(params)))
  await invalidateGitScope(params.projectId, params.workspaceId)
  return result
}

export async function gitUnstage(runtime: DesktopRuntime, params: GitPathsMutationParams): Promise<GitMutationResult> {
  const result = decodeProtocolJson<GitMutationResult>(await runtime.bridge.gitUnstage(JSON.stringify(params)))
  await invalidateGitScope(params.projectId, params.workspaceId)
  return result
}

export async function gitCommit(runtime: DesktopRuntime, params: GitCommitParams): Promise<GitMutationResult> {
  const result = decodeProtocolJson<GitMutationResult>(await runtime.bridge.gitCommit(JSON.stringify(params)))
  await invalidateGitScope(params.projectId, params.workspaceId)
  return result
}

export async function prepareGitCheckpointRevert(
  runtime: DesktopRuntime,
  params: GitCheckpointPrepareRevertParams,
): Promise<GitCheckpointPrepareRevertResult> {
  return decodeProtocolJson(await runtime.bridge.gitCheckpointPrepareRevert(JSON.stringify(params)))
}

export async function applyGitCheckpointRevert(
  runtime: DesktopRuntime,
  params: GitCheckpointApplyRevertParams,
  projectId: ProjectId,
  workspaceId: WorkspaceId,
): Promise<GitMutationResult> {
  const result = decodeProtocolJson<GitMutationResult>(
    await runtime.bridge.gitCheckpointApplyRevert(JSON.stringify(params)),
  )
  await invalidateWorkspaceAfterGitMutation(projectId, workspaceId)
  return result
}

export async function invalidateGitAfterRun(projectId: ProjectId, workspaceId: WorkspaceId): Promise<void> {
  await Promise.all([
    invalidateGitScope(projectId, workspaceId),
    desktopQueryClient.invalidateQueries({ queryKey: workspaceFilesQueryRoot }),
  ])
}
