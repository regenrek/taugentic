import type { DaemonEventCursor, SessionId } from "@taugentic/desktop-shared";

import { DaemonProtocolError } from "./daemon-rpc-connection.js";

type CursorLineageSource = "daemon.session.attach" | "daemon.subscribe";

type CursorLineageExpectation = {
  expectedSessionId: SessionId;
  expectedDaemonInstanceId: string;
};

export function assertDaemonCursorLineage(
  source: CursorLineageSource,
  cursor: DaemonEventCursor | null | undefined,
  expectation: CursorLineageExpectation,
): void {
  if (cursor == null) {
    return;
  }

  if (cursor.sessionId !== expectation.expectedSessionId) {
    throw new DaemonProtocolError(
      `${source} returned cursor for wrong session: expected ${expectation.expectedSessionId}, got ${cursor.sessionId}`,
    );
  }

  if (cursor.daemonInstanceId !== expectation.expectedDaemonInstanceId) {
    throw new DaemonProtocolError(
      `${source} returned cursor for wrong daemon instance: expected ${expectation.expectedDaemonInstanceId}, got ${cursor.daemonInstanceId}`,
    );
  }
}
