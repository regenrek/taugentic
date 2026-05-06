import type {
  AgentStreamMessage,
  ApprovalStreamMessage,
  ArtifactStreamMessage,
  ActivityCursor,
  DaemonEventCursor,
  DesktopStreamErrorListener,
  DesktopStreamListener,
  DesktopStreamUnsubscribe,
  RunStreamMessage,
  SessionId,
} from "@taugentic/desktop-shared";

export type StreamUnsubscribe = DesktopStreamUnsubscribe;

export function subscribeRunStream(
  sessionId: SessionId,
  afterCursor: ActivityCursor | null,
  onMessage: DesktopStreamListener<RunStreamMessage>,
  onError?: DesktopStreamErrorListener,
): Promise<DesktopStreamUnsubscribe> {
  return window.desktopStreams.subscribeRunStream(sessionId, afterCursor, onMessage, onError);
}

export function subscribeApprovalStream(
  sessionId: SessionId,
  afterCursor: DaemonEventCursor | null,
  onMessage: DesktopStreamListener<ApprovalStreamMessage>,
  onError?: DesktopStreamErrorListener,
): Promise<DesktopStreamUnsubscribe> {
  return window.desktopStreams.subscribeApprovalStream(sessionId, afterCursor, onMessage, onError);
}

export function subscribeArtifactStream(
  sessionId: SessionId,
  afterCursor: DaemonEventCursor | null,
  onMessage: DesktopStreamListener<ArtifactStreamMessage>,
  onError?: DesktopStreamErrorListener,
): Promise<DesktopStreamUnsubscribe> {
  return window.desktopStreams.subscribeArtifactStream(sessionId, afterCursor, onMessage, onError);
}

export function subscribeAgentStream(
  sessionId: SessionId,
  afterCursor: DaemonEventCursor | null,
  onMessage: DesktopStreamListener<AgentStreamMessage>,
  onError?: DesktopStreamErrorListener,
): Promise<DesktopStreamUnsubscribe> {
  return window.desktopStreams.subscribeAgentStream(sessionId, afterCursor, onMessage, onError);
}
