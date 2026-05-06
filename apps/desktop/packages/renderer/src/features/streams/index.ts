/*
 * features/streams Public API barrel.
 *
 * Existing transport helpers (connectHydratedStream + its dep contract) are
 * re-exported unchanged. The additive surface below is consumed by sibling
 * features that need a domain-event subscription primitive without poking
 * lib/ipc directly.
 *
 * The bus consumer (cortex-canvas/event-bus.ts) takes a StreamSubscriber as
 * a dependency. Construction of a concrete subscriber is the orchestrator's
 * job (e.g. shell or visualization panel wiring); this barrel only owns the
 * contract.
 */

import type { PublicDaemonEventEnvelope } from "@taugentic/desktop-shared";

export { createFocusedSessionSubscriber } from "./focused-session-subscribe.js";
export type {
  CreateFocusedSessionSubscriberOptions,
  FocusedSessionDomain,
  FocusedSessionTransport,
} from "./focused-session-subscribe.js";

/** Domain event seen by stream consumers. Kind discriminator lives on `event`. */
export type StreamEvent = PublicDaemonEventEnvelope;

/** Unsubscribe handle returned by {@link StreamSubscriber.subscribe}. */
export type StreamUnsubscribe = () => void;

/**
 * Minimal subscribe surface for cross-cutting consumers (e.g. cortex bus).
 *
 * Implementations may dispatch from any underlying transport (per-session
 * port, aggregated multiplexer, fixture). Multiple handlers are supported
 * so independent consumers do not race for the single underlying stream.
 */
export interface StreamSubscriber {
  subscribe(handler: (event: StreamEvent) => void): StreamUnsubscribe;
}
