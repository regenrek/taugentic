import type { ApprovalStreamMessage, SessionId } from "@taugentic/desktop-shared";

export interface SessionApprovalState {
  errorMessage: string | null;
  lastSequence: bigint | null;
  sessionId: SessionId;
  streamStatus: "connecting" | "ready" | "error";
}

export function createInitialSessionApprovalState(sessionId: SessionId): SessionApprovalState {
  return {
    errorMessage: null,
    lastSequence: null,
    sessionId,
    streamStatus: "connecting",
  };
}

export function reduceApprovalStreamMessage(
  state: SessionApprovalState,
  message: ApprovalStreamMessage,
): { needsRefresh: boolean; state: SessionApprovalState } {
  if ("status" in message) {
    switch (message.status) {
      case "historyGap":
        return {
          needsRefresh: true,
          state: {
            ...state,
            errorMessage: null,
            streamStatus: "ready",
          },
        };
      case "terminalError":
        return {
          needsRefresh: false,
          state: {
            ...state,
            errorMessage: `approval stream entered a terminal error state for ${state.sessionId}`,
            streamStatus: "error",
          },
        };
      case "ready":
        return {
          needsRefresh: false,
          state: {
            ...state,
            errorMessage: null,
            streamStatus: "ready",
          },
        };
    }
  }

  return {
    needsRefresh: true,
    state: {
      ...state,
      errorMessage: null,
      lastSequence: message.sequence,
      streamStatus: "ready",
    },
  };
}

export function toApprovalStreamErrorMessage(sessionId: SessionId, error: unknown): string {
  if (error instanceof Error) {
    return `approval stream failed for ${sessionId}: ${error.message}`;
  }

  return `approval stream failed for ${sessionId}: ${String(error)}`;
}
