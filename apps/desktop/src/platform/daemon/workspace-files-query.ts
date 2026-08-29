import { queryOptions } from "@tanstack/react-query"

import type { NativeImagePreview, ProjectId, WorkspaceFileReadResult, WorkspaceFileTreeResult, WorkspaceFileWriteResult, WorkspaceId } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"
import { decodeProtocolJson } from "./protocol-json.js"
import { desktopQueryClient } from "./query-client.js"

export const workspaceFilesQueryRoot = ["daemon", "workspace-files"] as const

export function workspaceFileTreeQueryKey(projectId: ProjectId, workspaceId: WorkspaceId) {
  return [...workspaceFilesQueryRoot, "tree", projectId, workspaceId] as const
}

export function workspaceFileReadQueryKey(projectId: ProjectId, workspaceId: WorkspaceId, path: string, pdfPageIndex?: number) {
  return [...workspaceFilesQueryRoot, "content", projectId, workspaceId, path, pdfPageIndex ?? "content"] as const
}

export function workspaceImagePreviewQueryKey(projectId: ProjectId, workspaceId: WorkspaceId, path: string) {
  return [...workspaceFilesQueryRoot, "image-preview", projectId, workspaceId, path] as const
}

export function workspaceFileTreeQuery(runtime: DesktopRuntime, projectId: ProjectId, workspaceId: WorkspaceId) {
  return queryOptions({
    queryKey: workspaceFileTreeQueryKey(projectId, workspaceId),
    queryFn: async (): Promise<WorkspaceFileTreeResult> => decodeProtocolJson(
      await runtime.bridge.workspaceFileTree(JSON.stringify({ projectId, workspaceId })),
    ),
  })
}

export function workspaceFileReadQuery(runtime: DesktopRuntime, projectId: ProjectId, workspaceId: WorkspaceId, path: string, pdfPageIndex?: number) {
  return queryOptions({
    queryKey: workspaceFileReadQueryKey(projectId, workspaceId, path, pdfPageIndex),
    queryFn: async (): Promise<WorkspaceFileReadResult> => decodeProtocolJson(
      await runtime.bridge.readWorkspaceFile(JSON.stringify({ projectId, workspaceId, path, ...(pdfPageIndex === undefined ? {} : { pdfPageIndex }) })),
    ),
  })
}

export function workspaceImagePreviewQuery(runtime: DesktopRuntime, projectId: ProjectId, workspaceId: WorkspaceId, path: string) {
  return queryOptions({
    queryKey: workspaceImagePreviewQueryKey(projectId, workspaceId, path),
    queryFn: async (): Promise<NativeImagePreview> => decodeProtocolJson(
      await runtime.bridge.materializeWorkspaceImage(JSON.stringify({ projectId, workspaceId, path })),
    ),
  })
}

export async function writeWorkspaceTextFile(runtime: DesktopRuntime, input: {
  projectId: ProjectId
  workspaceId: WorkspaceId
  path: string
  expectedRevision: string
  text: string
}): Promise<WorkspaceFileWriteResult> {
  const result = decodeProtocolJson<WorkspaceFileWriteResult>(await runtime.bridge.writeWorkspaceFile(JSON.stringify({
    ...input,
    userApproved: true,
  })))
  await Promise.all([
    desktopQueryClient.invalidateQueries({ queryKey: [...workspaceFilesQueryRoot, "content", input.projectId, input.workspaceId, input.path] }),
    desktopQueryClient.invalidateQueries({ queryKey: workspaceFileTreeQueryKey(input.projectId, input.workspaceId) }),
  ])
  return result
}

export async function openWorkspaceFileExternal(runtime: DesktopRuntime, input: {
  projectId: ProjectId
  workspaceId: WorkspaceId
  path: string
}): Promise<void> {
  await runtime.bridge.openWorkspaceFileExternal(JSON.stringify(input))
}
