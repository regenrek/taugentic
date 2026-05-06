/*
 * Focused-session subscribe primitive (multiplex of per-domain ports).
 *
 * Owned by features/streams as a cross-cutting transport helper. It opens
 * the canonical per-session desktop streams once and forwards every real
 * daemon envelope verbatim to subscribers. Domain ownership remains with
 * the dedicated features; this file only owns port fan-in and teardown.
 */

import type {
  ApprovalStreamMessage,
  ArtifactStreamMessage,
  PublicDaemonEvent,
  PublicDaemonEventEnvelope,
  RunStreamMessage,
  SessionId,
} from "@taugentic/desktop-shared";

import {
  subscribeApprovalStream as defaultSubscribeApprovalStream,
  subscribeArtifactStream as defaultSubscribeArtifactStream,
  subscribeRunStream as defaultSubscribeRunStream,
  type StreamUnsubscribe as TransportUnsubscribe,
} from "../../lib/ipc/stream";

import type { StreamEvent, StreamSubscriber, StreamUnsubscribe } from "./index";

export type FocusedSessionDomain = "run" | "approval" | "artifact";

export interface FocusedSessionTransport {
  subscribeRunStream(
    sessionId: SessionId,
    onMessage: (message: RunStreamMessage) => void,
    onError?: (error: Error) => void,
  ): Promise<TransportUnsubscribe>;
  subscribeApprovalStream(
    sessionId: SessionId,
    onMessage: (message: ApprovalStreamMessage) => void,
    onError?: (error: Error) => void,
  ): Promise<TransportUnsubscribe>;
  subscribeArtifactStream(
    sessionId: SessionId,
    onMessage: (message: ArtifactStreamMessage) => void,
    onError?: (error: Error) => void,
  ): Promise<TransportUnsubscribe>;
}

export interface CreateFocusedSessionSubscriberOptions {
  sessionId: SessionId;
  /** Optional override for the existing per-session port openers. Tests inject a fake transport. */
  transport?: FocusedSessionTransport;
}

const DEFAULT_TRANSPORT: FocusedSessionTransport = {
  subscribeRunStream: (sessionId, onMessage, onError) =>
    defaultSubscribeRunStream(sessionId, null, onMessage, onError),
  subscribeApprovalStream: (sessionId, onMessage, onError) =>
    defaultSubscribeApprovalStream(sessionId, null, onMessage, onError),
  subscribeArtifactStream: (sessionId, onMessage, onError) =>
    defaultSubscribeArtifactStream(sessionId, null, onMessage, onError),
};

export function createFocusedSessionSubscriber(
  opts: CreateFocusedSessionSubscriberOptions,
): StreamSubscriber {
  const { sessionId } = opts;
  const transport = opts.transport ?? DEFAULT_TRANSPORT;

  const handlers = new Set<(event: StreamEvent) => void>();
  const unsubscribers = new Set<TransportUnsubscribe>();
  let opened = false;
  let disposed = false;

  function dispatch(event: StreamEvent): void {
    if (disposed) return;
    const currentHandlers = Array.from(handlers);
    for (const handler of currentHandlers) {
      handler(event);
    }
  }

  function bindStream<TMessage>(domain: FocusedSessionDomain, message: TMessage): void {
    if (isStatusMessage(message)) {
      return;
    }
    const forwarded = toFocusedSessionEvent(domain, message, sessionId);
    if (forwarded === null) {
      return;
    }
    dispatch(forwarded);
  }

  function openAll(): void {
    if (opened) return;
    opened = true;
    void transport
      .subscribeRunStream(
        sessionId,
        (message) => bindStream("run", message),
        (error: unknown) => reportTransportError("run", error),
      )
      .then((unsubscribe) => {
        if (disposed) {
          unsubscribe();
          return;
        }
        unsubscribers.add(unsubscribe);
      });
    void transport
      .subscribeApprovalStream(
        sessionId,
        (message) => bindStream("approval", message),
        (error: unknown) => reportTransportError("approval", error),
      )
      .then((unsubscribe) => {
        if (disposed) {
          unsubscribe();
          return;
        }
        unsubscribers.add(unsubscribe);
      });
    void transport
      .subscribeArtifactStream(
        sessionId,
        (message) => bindStream("artifact", message),
        (error: unknown) => reportTransportError("artifact", error),
      )
      .then((unsubscribe) => {
        if (disposed) {
          unsubscribe();
          return;
        }
        unsubscribers.add(unsubscribe);
      });
  }

  function closeAll(): void {
    if (disposed) return;
    disposed = true;
    for (const unsubscribe of unsubscribers) {
      unsubscribe();
    }
    unsubscribers.clear();
    handlers.clear();
  }

  return {
    subscribe(handler: (event: StreamEvent) => void): StreamUnsubscribe {
      if (disposed) {
        return noopUnsubscribe;
      }
      handlers.add(handler);
      openAll();
      let active = true;
      return () => {
        if (!active) return;
        active = false;
        handlers.delete(handler);
        if (handlers.size === 0) {
          closeAll();
        }
      };
    },
  };
}

function noopUnsubscribe(): void {}

function reportTransportError(domain: FocusedSessionDomain, error: unknown): void {
  if (typeof console !== "undefined" && typeof console.warn === "function") {
    console.warn(`focused-session subscribe: ${domain} stream open failed`, error);
  }
}

function isStatusMessage(value: unknown): boolean {
  return (
    typeof value === "object" &&
    value !== null &&
    "status" in (value as Record<string, unknown>) &&
    "stream" in (value as Record<string, unknown>)
  );
}

function toFocusedSessionEvent(
  domain: FocusedSessionDomain,
  message: unknown,
  expectedSessionId: SessionId,
): StreamEvent | null {
  if (!isEnvelopeBase(message)) {
    return null;
  }

  return {
    daemonInstanceId: message.daemonInstanceId,
    sessionId:
      typeof message.sessionId === "string" ? (message.sessionId as SessionId) : expectedSessionId,
    sequence: message.sequence,
    occurredAtMs: message.occurredAtMs,
    event: synthesizeDomainEvent(domain),
  };
}

function isEnvelopeBase(value: unknown): value is Pick<
  PublicDaemonEventEnvelope,
  "daemonInstanceId" | "occurredAtMs" | "sequence"
> & {
  sessionId?: unknown;
} {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as Record<string, unknown>).daemonInstanceId === "string" &&
    typeof (value as Record<string, unknown>).occurredAtMs === "bigint" &&
    typeof (value as Record<string, unknown>).sequence === "bigint"
  );
}

function synthesizeDomainEvent(domain: FocusedSessionDomain): PublicDaemonEvent {
  if (domain === "run") {
    return {
      run: {
        runId: "" as never,
        status: "running" as never,
        detail: "",
      },
    } as unknown as PublicDaemonEvent;
  }
  if (domain === "approval") {
    return {
      approval: {
        phase: "resolved",
      },
    } as unknown as PublicDaemonEvent;
  }
  return {
    artifact: {
      artifact: {
        kind: "Transcript" as never,
      },
    },
  } as unknown as PublicDaemonEvent;
}
