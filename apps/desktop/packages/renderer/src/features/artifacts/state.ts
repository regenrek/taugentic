import type { ArtifactId, ArtifactStreamMessage, SessionId } from "@taugentic/desktop-shared";

export interface SessionArtifactState {
  currentArtifactId: ArtifactId | null;
  errorMessage: string | null;
  isHydrating: boolean;
  sessionId: SessionId;
  streamStatus: "connecting" | "live" | "error";
}

export function createInitialSessionArtifactState(sessionId: SessionId): SessionArtifactState {
  return {
    currentArtifactId: null,
    errorMessage: null,
    isHydrating: true,
    sessionId,
    streamStatus: "connecting",
  };
}

export function reduceArtifactStreamMessage(
  state: SessionArtifactState,
  message: ArtifactStreamMessage,
): { needsRefresh: boolean; state: SessionArtifactState } {
  if ("status" in message) {
    switch (message.status) {
      case "historyGap":
        return {
          needsRefresh: true,
          state: {
            ...state,
            errorMessage: null,
            isHydrating: true,
            streamStatus: "live",
          },
        };
      case "terminalError":
        return {
          needsRefresh: false,
          state: {
            ...state,
            errorMessage: `artifact stream entered a terminal error state for ${state.sessionId}`,
            isHydrating: false,
            streamStatus: "error",
          },
        };
      case "ready":
        return {
          needsRefresh: false,
          state: {
            ...state,
            errorMessage: null,
            streamStatus: "live",
          },
        };
    }
  }

  return {
    needsRefresh: true,
    state: {
      ...state,
      errorMessage: null,
      streamStatus: "live",
    },
  };
}

export function selectCurrentArtifact(
  state: SessionArtifactState,
  artifactId: ArtifactId,
): SessionArtifactState {
  return {
    ...state,
    currentArtifactId: artifactId,
  };
}

export function toArtifactStreamErrorMessage(sessionId: SessionId, error: unknown): string {
  if (error instanceof Error) {
    return `artifact refresh failed for ${sessionId}: ${error.message}`;
  }

  return `artifact refresh failed for ${sessionId}: ${String(error)}`;
}
