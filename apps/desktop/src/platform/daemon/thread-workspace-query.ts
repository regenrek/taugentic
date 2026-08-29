import { queryOptions } from "@tanstack/react-query"

import type { SessionId, ThreadWorkspaceMutation, ThreadWorkspaceResult } from "@taugentic/desktop-protocol"

import type { DesktopRuntime } from "./desktop-runtime.js"
import { decodeProtocolJson } from "./protocol-json.js"

/** Query cache identity only; the attached daemon session remains the durable owner. */
export const threadWorkspaceQueryRoot = ["daemon", "thread-workspace"] as const

export function threadWorkspaceQueryKey(sessionId: SessionId) {
  return [...threadWorkspaceQueryRoot, sessionId] as const
}

export function threadWorkspaceQuery(runtime: DesktopRuntime, sessionId: SessionId) {
  return queryOptions({
    queryKey: threadWorkspaceQueryKey(sessionId),
    queryFn: async (): Promise<ThreadWorkspaceResult> => decodeProtocolJson(
      await runtime.bridge.threadWorkspace(),
    ),
  })
}

export async function updateThreadWorkspace(
  runtime: DesktopRuntime,
  mutation: ThreadWorkspaceMutation,
): Promise<ThreadWorkspaceResult> {
  return decodeProtocolJson(await runtime.bridge.updateThreadWorkspace(JSON.stringify({ mutation })))
}
