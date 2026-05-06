import { useState } from "react";

import { useQueryClient } from "@tanstack/react-query";
import { useSelector } from "@xstate/react";

import type { AgentTurnRow, SessionId } from "@taugentic/desktop-shared";

import { useMountEffect } from "@/lib/react/use-mount-effect";
import { getAgentTurnsPage } from "@/lib/ipc/api";
import { subscribeAgentStream } from "@/lib/ipc/stream";
import { queryKeys } from "@/lib/queries/keys";
import { DEFAULT_AGENT_TURNS_PAGE_LIMIT } from "@/lib/queries/session-queries";

import {
  acquireAgentStreamSessionHandle,
  releaseAgentStreamSessionHandle,
  type SessionAgentStreamDeps,
} from "./connection";
import type { LiveAgentMessage, LiveAgentToolCall, SessionAgentStreamState } from "./state";

export type { SessionAgentStreamDeps } from "./connection";

export interface UseAgentStreamOptions {
  deps?: SessionAgentStreamDeps;
  limit?: number;
}

const defaultDeps = (qc: ReturnType<typeof useQueryClient>): SessionAgentStreamDeps => ({
  async loadCommitted(sessionId, limit) {
    const query = { limit } as const;
    const snapshot = await getAgentTurnsPage(sessionId, query);
    qc.setQueryData(queryKeys.sessionAgentTurns(sessionId, query), snapshot);
    return snapshot;
  },
  async subscribeAgentStream(sessionId, afterCursor, onMessage, onError) {
    return subscribeAgentStream(sessionId, afterCursor, onMessage, onError);
  },
});

export interface AgentStreamViewModel {
  committedRows: AgentTurnRow[];
  errorMessage: string | null;
  hasHydratedCommitted: boolean;
  liveMessages: LiveAgentMessage[];
  liveToolCalls: LiveAgentToolCall[];
  streamStatus: SessionAgentStreamState["streamStatus"];
}

export function useAgentStream(
  sessionId: SessionId,
  options: UseAgentStreamOptions = {},
): AgentStreamViewModel {
  const qc = useQueryClient();
  const deps = options.deps ?? defaultDeps(qc);
  const limit = options.limit ?? DEFAULT_AGENT_TURNS_PAGE_LIMIT;
  const [handle] = useState(() => acquireAgentStreamSessionHandle(sessionId, deps, limit));

  useMountEffect(() => () => releaseAgentStreamSessionHandle(handle));

  return useSelector(handle.actorRef, (snapshot) => ({
    committedRows: snapshot.context.committedRows,
    errorMessage: snapshot.context.errorMessage,
    hasHydratedCommitted: snapshot.context.hasHydratedCommitted,
    liveMessages: snapshot.context.liveMessages,
    liveToolCalls: snapshot.context.liveToolCalls,
    streamStatus: snapshot.context.streamStatus,
  }));
}
