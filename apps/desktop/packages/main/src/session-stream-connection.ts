import type { MessagePortMain } from "electron";

import type {
  AgentStreamEventEnvelope,
  ActivityCursor,
  ApprovalStreamEventEnvelope,
  ArtifactStreamEventEnvelope,
  DaemonEventCursor,
  DaemonEventEnvelope,
  DaemonEventKind,
  DaemonSubscribeResult,
  RunStreamEventEnvelope,
  SessionId,
} from "@taugentic/desktop-shared";
import { METHOD_DAEMON_SUBSCRIBE } from "@taugentic/desktop-shared";
import { parseDaemonSubscribeResult } from "@taugentic/desktop-shared/validation";

import { assertDaemonCursorLineage } from "./daemon-cursor-lineage.js";
import { DaemonSessionConnection } from "./daemon-session-connection.js";
import {
  DAEMON_REQUEST_TIMEOUT_STANDARD,
  DaemonProtocolError,
  isRetryablePersistentSessionError,
} from "./daemon-rpc-connection.js";

const RECONNECT_BASE_DELAY_MS = 250;
const RECONNECT_MAX_DELAY_MS = 5_000;

type StreamEventEnvelope =
  | AgentStreamEventEnvelope
  | RunStreamEventEnvelope
  | ApprovalStreamEventEnvelope
  | ArtifactStreamEventEnvelope;

type StreamKind = "run" | "approval" | "artifact" | "agentStream";
type InitialSubscribeCursor = ActivityCursor | DaemonEventCursor | null;
type NormalizedPortCursor = DaemonEventCursor | null;

type StreamStatus = "ready" | "historyGap" | "terminalError";

type PendingPortState = {
  afterCursor: NormalizedPortCursor;
  events: StreamEventEnvelope[];
};

type StreamDescriptor = {
  activePortCursors: Map<MessagePortMain, NormalizedPortCursor>;
  pendingPorts: Map<MessagePortMain, PendingPortState>;
  stream: "runs" | "approvals" | "artifacts" | "agentStream";
  ports: Set<MessagePortMain>;
  matches(envelope: DaemonEventEnvelope): boolean;
};

function toStreamEventEnvelope(envelope: DaemonEventEnvelope): StreamEventEnvelope {
  if ("agentStream" in envelope.event) {
    return envelope;
  }
  if ("run" in envelope.event) {
    return envelope;
  }
  if ("approval" in envelope.event) {
    return envelope;
  }
  if ("artifact" in envelope.event) {
    return envelope;
  }
  throw new Error("session stream connection received unsupported stream event");
}

export class SessionStreamConnection {
  constructor(
    private readonly attachedSessionId: SessionId,
    private readonly onIdle?: () => void,
  ) {
    this.connection = new DaemonSessionConnection(this.attachedSessionId, {
      requestTimeout: DAEMON_REQUEST_TIMEOUT_STANDARD,
      hooks: {
        onInitialize: (result) => this.handleDaemonEpochId(result.daemonInstanceId),
        onAttach: (result) => this.noteLatestCursor(result.latestCursor),
        onNotification: (envelope) => this.handleDaemonEventEnvelope(envelope),
        onTransportTermination: (error) => this.handleTransportTermination(error),
      },
    });
  }

  private readonly connection: DaemonSessionConnection;
  private subscribedKinds = new Set<DaemonEventKind>();
  private readonly runPorts = new Set<MessagePortMain>();
  private readonly approvalPorts = new Set<MessagePortMain>();
  private readonly artifactPorts = new Set<MessagePortMain>();
  private readonly agentStreamPorts = new Set<MessagePortMain>();
  private readonly streamDescriptors = {
    run: {
      activePortCursors: new Map<MessagePortMain, NormalizedPortCursor>(),
      pendingPorts: new Map<MessagePortMain, PendingPortState>(),
      stream: "runs",
      ports: this.runPorts,
      matches: (envelope: DaemonEventEnvelope) => "run" in envelope.event,
    },
    approval: {
      activePortCursors: new Map<MessagePortMain, NormalizedPortCursor>(),
      pendingPorts: new Map<MessagePortMain, PendingPortState>(),
      stream: "approvals",
      ports: this.approvalPorts,
      matches: (envelope: DaemonEventEnvelope) => "approval" in envelope.event,
    },
    artifact: {
      activePortCursors: new Map<MessagePortMain, NormalizedPortCursor>(),
      pendingPorts: new Map<MessagePortMain, PendingPortState>(),
      stream: "artifacts",
      ports: this.artifactPorts,
      matches: (envelope: DaemonEventEnvelope) => "artifact" in envelope.event,
    },
    agentStream: {
      activePortCursors: new Map<MessagePortMain, NormalizedPortCursor>(),
      pendingPorts: new Map<MessagePortMain, PendingPortState>(),
      stream: "agentStream",
      ports: this.agentStreamPorts,
      matches: (envelope: DaemonEventEnvelope) => "agentStream" in envelope.event,
    },
  } satisfies Record<StreamKind, StreamDescriptor>;
  private daemonInstanceId: string | null = null;
  private latestCursor: DaemonEventCursor | null = null;
  private pendingPortAttachments = 0;
  private handlingTermination = false;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempt = 0;

  async attachRunPort(
    port: MessagePortMain,
    afterCursor: ActivityCursor | null = null,
  ): Promise<void> {
    return this.attachPort("run", port, afterCursor);
  }

  async attachApprovalPort(
    port: MessagePortMain,
    afterCursor: DaemonEventCursor | null = null,
  ): Promise<void> {
    return this.attachPort("approval", port, afterCursor);
  }

  async attachArtifactPort(
    port: MessagePortMain,
    afterCursor: DaemonEventCursor | null = null,
  ): Promise<void> {
    return this.attachPort("artifact", port, afterCursor);
  }

  async attachAgentStreamPort(
    port: MessagePortMain,
    afterCursor: DaemonEventCursor | null = null,
  ): Promise<void> {
    return this.attachPort("agentStream", port, afterCursor);
  }

  private async attachPort(
    kind: StreamKind,
    port: MessagePortMain,
    afterCursor: InitialSubscribeCursor = null,
  ): Promise<void> {
    const descriptor = this.streamDescriptors[kind];
    const pendingAfterCursor = this.tryNormalizePortCursor(afterCursor);
    const isClosed = this.bindPortLifecycle(descriptor.ports, port);
    descriptor.pendingPorts.set(port, {
      afterCursor: pendingAfterCursor,
      events: [],
    });
    this.beginPortAttachment();
    try {
      const status = await this.ensureSubscribedKinds([kind], afterCursor);
      if (isClosed()) {
        return;
      }
      port.postMessage(this.createStreamStatus(descriptor.stream, status));
      if (!isClosed()) {
        descriptor.ports.add(port);
        descriptor.activePortCursors.set(port, this.toInitialSubscribeCursor(afterCursor));
        if (status === "ready") {
          this.flushPendingPortEvents(descriptor, port);
        }
      }
    } finally {
      descriptor.pendingPorts.delete(port);
      this.endPortAttachment();
    }
  }

  private bindPortLifecycle(ports: Set<MessagePortMain>, port: MessagePortMain): () => boolean {
    let closed = false;
    port.once("close", () => {
      closed = true;
      for (const descriptor of Object.values(this.streamDescriptors)) {
        descriptor.pendingPorts.delete(port);
        descriptor.activePortCursors.delete(port);
      }
      ports.delete(port);
      this.stopReconnectWhenIdle();
    });
    return () => closed;
  }

  private async ensureConnected(): Promise<void> {
    return this.connection.ensureConnected();
  }

  private handleDaemonEpochId(daemonInstanceId: string): void {
    if (this.daemonInstanceId === daemonInstanceId) {
      return;
    }

    this.daemonInstanceId = daemonInstanceId;
    this.resetEventState();
  }

  private async ensureSubscribedKinds(
    kinds: DaemonEventKind[],
    afterCursor: InitialSubscribeCursor = null,
  ): Promise<StreamStatus> {
    return this.enqueueOperation(async () => {
      await this.ensureConnected();
      const missingKinds = kinds.filter((kind) => !this.subscribedKinds.has(kind));
      if (missingKinds.length === 0) {
        return "ready";
      }

      const result = await this.subscribeToDaemonEvents(
        missingKinds,
        this.toInitialSubscribeCursor(afterCursor),
      );
      for (const kind of missingKinds) {
        this.subscribedKinds.add(kind);
      }
      return result.status;
    });
  }

  private async subscribeToDaemonEvents(
    kinds: DaemonEventKind[],
    afterCursor?: DaemonEventCursor | null,
  ): Promise<DaemonSubscribeResult> {
    const result = await this.requestDaemonEventSubscription(kinds, afterCursor);
    this.assertCurrentCursorLineage("daemon.subscribe", result.latestCursor);
    this.noteLatestCursor(result.latestCursor);
    return result;
  }

  private requestDaemonEventSubscription(
    kinds: DaemonEventKind[],
    afterCursor?: DaemonEventCursor | null,
  ): Promise<DaemonSubscribeResult> {
    return this.sendRequest(
      METHOD_DAEMON_SUBSCRIBE,
      {
        kinds,
        afterCursor:
          afterCursor == null
            ? undefined
            : {
                daemonInstanceId: afterCursor.daemonInstanceId,
                sessionId: afterCursor.sessionId,
                sequence: afterCursor.sequence.toString(),
              },
      },
      parseDaemonSubscribeResult,
    );
  }

  private async restoreSubscriptions(): Promise<void> {
    const kinds = this.activeKinds();
    if (kinds.length === 0) {
      this.resetReconnectBackoff();
      return;
    }

    return this.enqueueOperation(async () => {
      await this.ensureConnected();
      const result = await this.requestDaemonEventSubscription(kinds, this.latestCursor);
      this.assertCurrentCursorLineage("daemon.subscribe", result.latestCursor);
      this.noteLatestCursor(result.latestCursor);
      for (const kind of kinds) {
        this.subscribedKinds.add(kind);
      }
      if (result.status === "historyGap") {
        this.postSubscriptionStatus(kinds, "historyGap");
      }
      this.resetReconnectBackoff();
    });
  }

  private postSubscriptionStatus(kinds: StreamKind[], status: StreamStatus): void {
    for (const kind of kinds) {
      const descriptor = this.streamDescriptors[kind];
      for (const port of descriptor.ports) {
        port.postMessage(this.createStreamStatus(descriptor.stream, status));
      }
    }
  }

  private createStreamStatus(
    stream: StreamDescriptor["stream"],
    status: StreamStatus,
  ): {
    latestCursor?: DaemonEventCursor | null;
    status: StreamStatus;
    stream: StreamDescriptor["stream"];
  } {
    if (stream !== "agentStream" || this.latestCursor === null) {
      return { stream, status };
    }
    return {
      latestCursor: this.latestCursor,
      stream,
      status,
    };
  }

  private enqueueOperation<Result>(operation: () => Promise<Result>): Promise<Result> {
    return this.connection.enqueueOperation(operation);
  }

  private sendRequest<Result>(
    method: string,
    params: Record<string, unknown>,
    parseResult: (value: unknown) => Result,
  ): Promise<Result> {
    return this.connection.request(method, params, parseResult);
  }

  private handleDaemonEventEnvelope(envelope: DaemonEventEnvelope): void {
    const isNewEvent = this.recordEvent(envelope);
    if (!isNewEvent) {
      return;
    }

    this.dispatchEventEnvelope(envelope);
  }

  private recordEvent(envelope: DaemonEventEnvelope): boolean {
    this.assertCurrentEventLineage(envelope);

    if (
      this.latestCursor !== null &&
      this.latestCursor.daemonInstanceId === envelope.daemonInstanceId &&
      this.latestCursor.sessionId === envelope.sessionId &&
      envelope.sequence <= this.latestCursor.sequence
    ) {
      return false;
    }

    this.noteLatestCursor({
      daemonInstanceId: envelope.daemonInstanceId,
      sessionId: envelope.sessionId,
      sequence: envelope.sequence,
    });
    return true;
  }

  private noteLatestCursor(cursor: DaemonEventCursor | null | undefined): void {
    if (cursor == null) {
      return;
    }

    if (cursor.sessionId !== this.attachedSessionId) {
      return;
    }
    if (this.daemonInstanceId !== null && cursor.daemonInstanceId !== this.daemonInstanceId) {
      return;
    }

    if (
      this.latestCursor === null ||
      this.latestCursor.daemonInstanceId !== cursor.daemonInstanceId ||
      this.latestCursor.sessionId !== cursor.sessionId ||
      cursor.sequence > this.latestCursor.sequence
    ) {
      this.latestCursor = cursor;
    }
  }

  private resetEventState(): void {
    this.latestCursor = null;
  }

  private toInitialSubscribeCursor(afterCursor: InitialSubscribeCursor): DaemonEventCursor | null {
    if (afterCursor == null) {
      return null;
    }

    if ("daemonInstanceId" in afterCursor) {
      return afterCursor;
    }

    return {
      daemonInstanceId: this.requireDaemonInstanceId(),
      sessionId: this.attachedSessionId,
      sequence: afterCursor.sequence,
    };
  }

  private assertCurrentCursorLineage(
    source: "daemon.subscribe",
    cursor: DaemonEventCursor | null | undefined,
  ): void {
    assertDaemonCursorLineage(source, cursor, {
      expectedSessionId: this.attachedSessionId,
      expectedDaemonInstanceId: this.requireDaemonInstanceId(),
    });
  }

  private assertCurrentEventLineage(envelope: DaemonEventEnvelope): void {
    if (envelope.sessionId !== this.attachedSessionId) {
      throw new DaemonProtocolError(
        `daemon.event returned envelope for wrong session: expected ${this.attachedSessionId}, got ${envelope.sessionId}`,
      );
    }

    const daemonInstanceId = this.requireDaemonInstanceId();
    if (envelope.daemonInstanceId !== daemonInstanceId) {
      throw new DaemonProtocolError(
        `daemon.event returned envelope for wrong daemon instance: expected ${daemonInstanceId}, got ${envelope.daemonInstanceId}`,
      );
    }
  }

  private requireDaemonInstanceId(): string {
    if (this.daemonInstanceId == null) {
      throw new DaemonProtocolError(
        "daemon.subscribe restore requires an initialized daemon instance id",
      );
    }
    return this.daemonInstanceId;
  }

  private dispatchEventEnvelope(envelope: DaemonEventEnvelope): void {
    for (const descriptor of Object.values(this.streamDescriptors)) {
      if (!descriptor.matches(envelope)) {
        continue;
      }
      const streamEnvelope = toStreamEventEnvelope(envelope);
      this.postToPorts(descriptor, streamEnvelope);
      this.bufferPendingPortEvent(descriptor, streamEnvelope);
      return;
    }
  }

  private bufferPendingPortEvent(
    descriptor: StreamDescriptor,
    envelope: StreamEventEnvelope,
  ): void {
    for (const pendingPortState of descriptor.pendingPorts.values()) {
      if (!this.shouldDeliverEnvelope(envelope, pendingPortState.afterCursor)) {
        continue;
      }
      pendingPortState.events.push(envelope);
    }
  }

  private flushPendingPortEvents(descriptor: StreamDescriptor, port: MessagePortMain): void {
    const pendingPortState = descriptor.pendingPorts.get(port);
    if (!pendingPortState || pendingPortState.events.length === 0) {
      return;
    }

    for (const envelope of pendingPortState.events) {
      port.postMessage(envelope);
    }
    pendingPortState.events.length = 0;
  }

  private postToPorts(descriptor: StreamDescriptor, message: StreamEventEnvelope): void {
    for (const port of descriptor.ports) {
      if (!this.shouldDeliverEnvelope(message, descriptor.activePortCursors.get(port) ?? null)) {
        continue;
      }
      port.postMessage(message);
    }
  }

  private shouldDeliverEnvelope(
    envelope: StreamEventEnvelope,
    afterCursor: NormalizedPortCursor,
  ): boolean {
    if (afterCursor == null) {
      return true;
    }
    if (envelope.sessionId !== afterCursor.sessionId) {
      return false;
    }
    if (envelope.daemonInstanceId !== afterCursor.daemonInstanceId) {
      return true;
    }
    return envelope.sequence > afterCursor.sequence;
  }

  private tryNormalizePortCursor(afterCursor: InitialSubscribeCursor): NormalizedPortCursor {
    if (afterCursor == null) {
      return null;
    }
    if ("daemonInstanceId" in afterCursor) {
      return afterCursor;
    }
    if (this.daemonInstanceId == null) {
      return null;
    }
    return {
      daemonInstanceId: this.daemonInstanceId,
      sessionId: this.attachedSessionId,
      sequence: afterCursor.sequence,
    };
  }

  private handleTransportTermination(_error: unknown): void {
    const error = _error;
    if (this.handlingTermination) {
      return;
    }
    this.handlingTermination = true;
    const activePortCount = this.activePortCount();
    this.subscribedKinds.clear();

    this.handlingTermination = false;
    if (activePortCount === 0) {
      this.resetReconnectBackoff();
      return;
    }

    if (!isRetryablePersistentSessionError(error)) {
      this.stopRestoringSubscriptions(
        this.activeKinds(),
        error,
        "stopped persistent daemon subscriptions after terminal failure",
      );
      return;
    }

    this.scheduleRestoreSubscriptions();
  }

  private scheduleRestoreSubscriptions(): void {
    if (this.reconnectTimer || this.activePortCount() === 0) {
      return;
    }

    const delay = Math.min(
      RECONNECT_BASE_DELAY_MS * 2 ** this.reconnectAttempt,
      RECONNECT_MAX_DELAY_MS,
    );
    this.reconnectAttempt += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      const activeKinds = this.activeKinds();
      void this.restoreSubscriptions().catch((restoreError: unknown) => {
        if (!isRetryablePersistentSessionError(restoreError)) {
          this.stopRestoringSubscriptions(
            activeKinds,
            restoreError,
            "stopped restoring persistent daemon subscriptions after terminal failure",
          );
          return;
        }
        console.error("failed to restore persistent daemon subscriptions", restoreError);
        this.scheduleRestoreSubscriptions();
      });
    }, delay);
  }

  private resetReconnectBackoff(): void {
    this.reconnectAttempt = 0;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private stopReconnectWhenIdle(): void {
    if (this.activePortCount() === 0) {
      this.resetReconnectBackoff();
      this.onIdle?.();
    }
  }

  private stopRestoringSubscriptions(
    kinds: StreamKind[],
    error: unknown,
    logMessage: string,
  ): void {
    this.connection.dispose();
    this.postSubscriptionStatus(kinds, "terminalError");
    this.resetReconnectBackoff();
    console.error(logMessage, error);
  }

  private activePortCount(): number {
    return (
      this.activeKinds().reduce(
        (count, kind) => count + this.streamDescriptors[kind].ports.size,
        0,
      ) + this.pendingPortAttachments
    );
  }

  private activeKinds(): StreamKind[] {
    return (Object.entries(this.streamDescriptors) as Array<[StreamKind, StreamDescriptor]>)
      .filter(([, descriptor]) => descriptor.ports.size > 0)
      .map(([kind]) => kind);
  }

  private beginPortAttachment(): void {
    this.pendingPortAttachments += 1;
  }

  private endPortAttachment(): void {
    this.pendingPortAttachments -= 1;
    this.stopReconnectWhenIdle();
  }

  dispose(): void {
    this.resetReconnectBackoff();
    this.subscribedKinds.clear();
    this.connection.dispose();
  }
}
