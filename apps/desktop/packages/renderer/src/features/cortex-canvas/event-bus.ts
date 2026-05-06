/*
 * Cortex event bus.
 *
 * Thin adapter that consumes the features/streams Public API
 * (StreamSubscriber yielding PublicDaemonEventEnvelope) and translates
 * each envelope into one engine call:
 *   - spawnParticle for generic per-session activity / cancelled / queued
 *   - triggerAssemblyBloom for completed states + finalized artifacts
 *   - triggerPulseRing 'attention' for approval requests / waitingForApproval
 *   - triggerPulseRing 'failed' (+ optional lane failed-border) for failures
 *
 * Throughput is observed across ALL events (regardless of focused session)
 * to drive idle breath modulation; spawn/pulse/bloom calls themselves are
 * gated on the focused session at handler-call time.
 *
 * Plain TypeScript only -- no React, no DOM. Lane row class toggling is
 * delegated to the visualization panel via opts.onLaneEffect.
 */

import type { StreamEvent, StreamSubscriber } from "../streams/index.js";
import type { BloomGlyph, CortexEngine, PulseTone } from "./public-api.js";

const BREATH_THROTTLE_MS = 250;
const BREATH_WINDOW_MS = 1_000;
const BREATH_BASE_HZ = 0.6;
const BREATH_PER_EPS = 0.05;
const BREATH_MAX_DELTA_HZ = 1.4;

export type LaneEffect = "sweep" | "failed-border";

export interface CortexBusOptions {
  engine: CortexEngine;
  streams: StreamSubscriber;
  /** Reactive accessor, read inside the handler so updates apply immediately. */
  focusedSessionId(this: void): string | null;
  /** Optional callback for non-engine lane row side effects. */
  onLaneEffect?(this: void, args: { laneId: string; effect: LaneEffect }): void;
}

export interface CortexBusHandle {
  dispose(): void;
}

interface MappedEffect {
  spawn?: { intensity: number };
  bloom?: { glyph: BloomGlyph };
  pulse?: { tone: PulseTone };
  laneEffect?: LaneEffect;
}

export function createCortexBus(opts: CortexBusOptions): CortexBusHandle {
  const engine = opts.engine;
  const streams = opts.streams;
  const focusedSessionId = opts.focusedSessionId;
  const onLaneEffect = opts.onLaneEffect;

  let disposed = false;
  let unsubscribe: (() => void) | null = null;
  const eventTimestampsMs: number[] = [];
  let lastBreathUpdateMs = Number.NEGATIVE_INFINITY;

  const handler = (event: StreamEvent): void => {
    if (disposed) return;

    const nowMs = nowMillis();
    recordThroughput(nowMs);
    maybeUpdateBreath(nowMs);

    const laneId = String(event.sessionId);
    const focused = focusedSessionId();
    if (focused === null || focused === undefined || laneId !== focused) {
      return;
    }

    const mapped = mapEnvelope(event);
    if (mapped.spawn) {
      engine.spawnParticle({ laneId, intensity: mapped.spawn.intensity });
    }
    if (mapped.bloom) {
      engine.triggerAssemblyBloom({ laneId, glyph: mapped.bloom.glyph });
    }
    if (mapped.pulse) {
      engine.triggerPulseRing({ anchorId: laneId, tone: mapped.pulse.tone });
    }
    if (mapped.laneEffect && onLaneEffect) {
      onLaneEffect({ laneId, effect: mapped.laneEffect });
    }
  };

  function recordThroughput(nowMs: number): void {
    eventTimestampsMs.push(nowMs);
    const cutoff = nowMs - BREATH_WINDOW_MS;
    while (eventTimestampsMs.length > 0 && eventTimestampsMs[0]! < cutoff) {
      eventTimestampsMs.shift();
    }
  }

  function maybeUpdateBreath(nowMs: number): void {
    if (nowMs - lastBreathUpdateMs < BREATH_THROTTLE_MS) return;
    lastBreathUpdateMs = nowMs;
    const eventsPerSec = eventTimestampsMs.length;
    const delta = Math.min(BREATH_MAX_DELTA_HZ, eventsPerSec * BREATH_PER_EPS);
    engine.setBreath({ hz: BREATH_BASE_HZ + delta });
  }

  unsubscribe = streams.subscribe(handler);

  return {
    dispose(): void {
      if (disposed) return;
      disposed = true;
      const fn = unsubscribe;
      unsubscribe = null;
      if (fn) {
        try {
          fn();
        } catch {
          // Subscriber teardown errors must not propagate; the bus is
          // a best-effort adapter and the underlying transport owns its own
          // error reporting.
        }
      }
      eventTimestampsMs.length = 0;
    },
  };
}

function mapEnvelope(envelope: StreamEvent): MappedEffect {
  const event = envelope.event;
  if ("session" in event) return mapSessionStatus(event.session.status);
  if ("run" in event) return mapRunStatus(event.run.status);
  if ("approval" in event) return mapApprovalPhase(event.approval.phase);
  if ("artifact" in event) return mapArtifactKind(event.artifact.artifact.kind);
  return {};
}

function mapSessionStatus(status: string): MappedEffect {
  switch (status) {
    case "failed":
      return { pulse: { tone: "failed" }, laneEffect: "failed-border" };
    case "completed":
      return { bloom: { glyph: "ok" } };
    case "running":
    case "paused":
    case "idle":
    default:
      return { spawn: { intensity: 1 } };
  }
}

function mapRunStatus(status: string): MappedEffect {
  switch (status) {
    case "failed":
      return { pulse: { tone: "failed" }, laneEffect: "failed-border" };
    case "completed":
      return { bloom: { glyph: "ok" } };
    case "waitingForApproval":
      return { pulse: { tone: "attention" } };
    case "queued":
    case "running":
    case "cancelled":
    default:
      return { spawn: { intensity: 1 } };
  }
}

function mapApprovalPhase(phase: string): MappedEffect {
  if (phase === "requested") return { pulse: { tone: "attention" } };
  return { bloom: { glyph: "ok" } };
}

function mapArtifactKind(kind: string): MappedEffect {
  switch (kind) {
    case "Patch":
    case "FileSnapshot":
      return { bloom: { glyph: "diff" }, laneEffect: "sweep" };
    case "CommandLog":
      return { bloom: { glyph: "tool" }, laneEffect: "sweep" };
    case "Transcript":
    default:
      return { bloom: { glyph: "ok" } };
  }
}

function nowMillis(): number {
  const perf = (globalThis as { performance?: { now(): number } }).performance;
  if (perf && typeof perf.now === "function") return perf.now();
  return Date.now();
}
