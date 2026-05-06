import type {
  DaemonEventEnvelope,
  DaemonInitializeResult,
  DaemonSessionAttachResult,
  RunEventStreamItem,
  SessionId,
} from "@taugentic/desktop-shared";
import {
  METHOD_DAEMON_INITIALIZE,
  METHOD_DAEMON_SESSION_ATTACH,
  PROTOCOL_VERSION,
} from "@taugentic/desktop-shared";
import {
  parseDaemonInitializeResult,
  parseDaemonSessionAttachResult,
} from "@taugentic/desktop-shared/validation";

import { assertDaemonCursorLineage } from "./daemon-cursor-lineage.js";
import {
  loadDesktopClientCredential,
  storeDesktopClientCredential,
} from "./daemon-client-credential.js";
import {
  loadDesktopSessionAuthority,
  removeDesktopClientSessionAuthorities,
  removeDesktopSessionAuthority,
  storeDesktopSessionAuthority,
} from "./daemon-session-authority.js";
import type { DaemonRequestTimeoutPolicy } from "./daemon-rpc-connection.js";
import {
  DaemonJsonRpcError,
  DaemonProtocolError,
  DaemonRpcConnection,
} from "./daemon-rpc-connection.js";
import { openDaemonRpcSocket } from "./daemon-rpc-client.js";

interface DaemonSessionConnectionHooks {
  onInitialize?: (result: DaemonInitializeResult) => void;
  onAttach?: (result: DaemonSessionAttachResult) => void;
  onNotification?: (envelope: DaemonEventEnvelope) => void;
  onRunEventNotification?: (item: RunEventStreamItem) => void;
  onTransportTermination?: (error: unknown) => void;
}

interface DaemonSessionConnectionOptions {
  hooks?: DaemonSessionConnectionHooks;
  requestTimeout: DaemonRequestTimeoutPolicy;
}

export class DaemonSessionConnection {
  private readonly hooks: DaemonSessionConnectionHooks;

  constructor(
    private readonly attachedSessionId: SessionId | null,
    options: DaemonSessionConnectionOptions,
  ) {
    this.hooks = options.hooks ?? {};
    this.rpcConnection = new DaemonRpcConnection({
      initializeConnection: () => this.initializeConnection(),
      onNotification: (envelope) => this.hooks.onNotification?.(envelope),
      onRunEventNotification: (item) => this.hooks.onRunEventNotification?.(item),
      openSocket: openDaemonRpcSocket,
      onTransportTermination: (error) => this.hooks.onTransportTermination?.(error),
      requestTimeout: options.requestTimeout,
    });
  }

  private readonly rpcConnection: DaemonRpcConnection;

  ensureConnected(): Promise<void> {
    return this.rpcConnection.ensureConnected();
  }

  enqueueOperation<Result>(operation: () => Promise<Result>): Promise<Result> {
    return this.rpcConnection.enqueueOperation(operation);
  }

  request<Result>(
    method: string,
    params: Record<string, unknown>,
    parseResult: (value: unknown) => Result,
  ): Promise<Result> {
    return this.rpcConnection.request(method, params, parseResult);
  }

  async initializeConnection(): Promise<void> {
    const clientCredential = await loadDesktopClientCredential("desktop-main");
    const initializeResult = await this.request(
      METHOD_DAEMON_INITIALIZE,
      {
        clientName: "desktop-main",
        clientCredential,
        clientVersion: "0.0.1",
        protocolVersion: PROTOCOL_VERSION,
        capabilities: {
          notifications: true,
          eventSubscriptions: true,
        },
      },
      parseDaemonInitializeResult,
    );
    this.assertProtocolVersion(initializeResult);
    if (clientCredential !== initializeResult.clientCredential) {
      await removeDesktopClientSessionAuthorities("desktop-main");
    }
    await storeDesktopClientCredential("desktop-main", initializeResult.clientCredential);
    this.hooks.onInitialize?.(initializeResult);

    if (this.attachedSessionId === null) {
      return;
    }
    const attachedSessionId = this.attachedSessionId;

    const sessionAuthority = await loadDesktopSessionAuthority("desktop-main", attachedSessionId);
    if (sessionAuthority === null) {
      throw new DaemonProtocolError(`missing local session authority for ${attachedSessionId}`);
    }

    const attachResult = await this.request(
      METHOD_DAEMON_SESSION_ATTACH,
      { sessionId: attachedSessionId, sessionAuthority },
      parseDaemonSessionAttachResult,
    ).catch(async (error: unknown) => {
      if (isRejectedSessionAuthorityError(error, attachedSessionId)) {
        await removeDesktopSessionAuthority("desktop-main", attachedSessionId);
      }
      throw error;
    });
    this.assertAttachedSessionResult(attachResult, initializeResult.daemonInstanceId);
    await storeDesktopSessionAuthority(
      "desktop-main",
      attachedSessionId,
      attachResult.sessionAuthority,
    );
    this.hooks.onAttach?.(attachResult);
  }

  dispose(): void {
    this.rpcConnection.dispose();
  }

  private assertProtocolVersion(result: DaemonInitializeResult): void {
    if (result.protocolVersion !== PROTOCOL_VERSION) {
      throw new DaemonProtocolError(
        `daemon protocol mismatch: expected ${PROTOCOL_VERSION}, got ${result.protocolVersion}`,
      );
    }
  }

  private assertAttachedSessionResult(
    result: DaemonSessionAttachResult,
    daemonInstanceId: string,
  ): void {
    if (this.attachedSessionId === null) {
      return;
    }

    if (result.session.id !== this.attachedSessionId) {
      throw new DaemonProtocolError(
        `daemon attached wrong session: expected ${this.attachedSessionId}, got ${result.session.id}`,
      );
    }

    assertDaemonCursorLineage("daemon.session.attach", result.latestCursor, {
      expectedSessionId: this.attachedSessionId,
      expectedDaemonInstanceId: daemonInstanceId,
    });
  }
}

function isRejectedSessionAuthorityError(error: unknown, sessionId: SessionId): boolean {
  return (
    error instanceof DaemonJsonRpcError &&
    error.code === -32_602 &&
    (error.rpcMessage === `session does not exist: ${sessionId}` ||
      error.rpcMessage === `session authority rejected: ${sessionId}`)
  );
}
