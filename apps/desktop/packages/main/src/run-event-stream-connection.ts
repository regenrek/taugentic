import type { MessagePortMain } from "electron";

import type {
  RunEventStreamItem,
  RunEventStreamStatus,
  RunId,
  SessionId,
  SubscribeRunEventsResult,
} from "@taugentic/desktop-shared";
import { METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS } from "@taugentic/desktop-shared";
import { parseSubscribeRunEventsResult } from "@taugentic/desktop-shared/validation";

import { DaemonSessionConnection } from "./daemon-session-connection.js";
import { DAEMON_REQUEST_TIMEOUT_STANDARD } from "./daemon-rpc-connection.js";

export class RunEventStreamConnection {
  constructor(
    private readonly sessionId: SessionId,
    private readonly runId: RunId,
    private readonly port: MessagePortMain,
    private readonly afterSeq: bigint | null,
    private readonly onClosed: () => void,
  ) {
    this.connection = new DaemonSessionConnection(sessionId, {
      requestTimeout: DAEMON_REQUEST_TIMEOUT_STANDARD,
      hooks: {
        onRunEventNotification: (item) => this.handleRunEventItem(item),
        onTransportTermination: () => this.postStatus("terminalError", null),
      },
    });
    port.once("close", () => this.dispose());
  }

  private readonly connection: DaemonSessionConnection;
  private disposed = false;

  async open(): Promise<void> {
    await this.connection.ensureConnected();
    if (this.disposed) {
      return;
    }
    const replay = await this.connection.request(
      METHOD_DAEMON_RUN_SUBSCRIBE_EVENTS,
      {
        sessionId: this.sessionId,
        runId: this.runId,
        afterSeq: this.afterSeq === null ? undefined : this.afterSeq.toString(),
      },
      parseSubscribeRunEventsResult,
    );
    if (this.disposed) {
      return;
    }
    this.postStatus("ready", replay.latestEventSeq ?? null);
    this.postReplay(replay);
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.connection.dispose();
    this.onClosed();
  }

  private handleRunEventItem(item: RunEventStreamItem): void {
    if (this.disposed || item.runId !== this.runId) {
      return;
    }
    this.port.postMessage(item);
    if ("error" in item.payload) {
      this.dispose();
    }
  }

  private postReplay(replay: SubscribeRunEventsResult): void {
    for (const delta of replay.events) {
      this.port.postMessage({
        runId: this.runId,
        payload: {
          kind: "delta",
          delta,
        },
      } satisfies RunEventStreamItem);
    }
  }

  private postStatus(status: RunEventStreamStatus["status"], latestEventSeq: bigint | null): void {
    if (this.disposed) {
      return;
    }
    this.port.postMessage({
      latestEventSeq,
      stream: "runEvents",
      status,
    } satisfies RunEventStreamStatus);
  }
}
