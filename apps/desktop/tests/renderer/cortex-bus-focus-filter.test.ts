import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { createCortexBus } from "../../packages/renderer/src/features/cortex-canvas/event-bus.js";
import type {
  StreamEvent,
  StreamSubscriber,
} from "../../packages/renderer/src/features/streams/index.js";
import { makeCortexEngineSpy, type RestorableSpy } from "./support/cortex-engine-spy.js";

function makeSubscriber(): StreamSubscriber & {
  dispatch(e: StreamEvent): void;
  unsubscribe: ReturnType<typeof vi.fn<() => void>>;
} {
  let handler: ((e: StreamEvent) => void) | null = null;
  const unsubscribe = vi.fn<() => void>();
  return {
    subscribe(h) {
      handler = h;
      return unsubscribe;
    },
    dispatch(e) {
      handler?.(e);
    },
    unsubscribe,
  };
}

function envelope(
  sessionId: string,
  runStatus: "running" | "waitingForApproval" | "completed" | "failed",
): StreamEvent {
  return {
    daemonInstanceId: "d",
    sessionId,
    sequence: 0n,
    occurredAtMs: 0n,
    event: { run: { runId: "r-1", status: runStatus, detail: "" } },
  };
}

let nowSpy: RestorableSpy | null = null;
let nowValue = 0;

beforeEach(() => {
  nowValue = 0;
  nowSpy = vi.spyOn(globalThis.performance, "now").mockImplementation(() => nowValue);
});

afterEach(() => {
  nowSpy?.mockRestore();
  nowSpy = null;
});

describe("cortex bus focus filter", () => {
  it("drops spawn/pulse/bloom calls for non-focused sessions", () => {
    const engine = makeCortexEngineSpy();
    const streams = makeSubscriber();
    const onLaneEffect = vi.fn();
    createCortexBus({
      engine,
      streams,
      focusedSessionId: () => "session-A",
      onLaneEffect,
    });

    streams.dispatch(envelope("session-B", "running"));
    streams.dispatch(envelope("session-B", "waitingForApproval"));
    streams.dispatch(envelope("session-B", "completed"));
    streams.dispatch(envelope("session-B", "failed"));

    expect(engine.spawnParticle).not.toHaveBeenCalled();
    expect(engine.triggerPulseRing).not.toHaveBeenCalled();
    expect(engine.triggerAssemblyBloom).not.toHaveBeenCalled();
    expect(onLaneEffect).not.toHaveBeenCalled();
  });

  it("still updates breath modulation for non-focused sessions", () => {
    const engine = makeCortexEngineSpy();
    const streams = makeSubscriber();
    createCortexBus({
      engine,
      streams,
      focusedSessionId: () => "session-A",
    });

    nowValue = 0;
    streams.dispatch(envelope("session-B", "running"));
    nowValue = 300;
    streams.dispatch(envelope("session-B", "running"));

    expect(engine.setBreath.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it("treats null focusedSessionId as 'no focus' and drops engine calls but keeps breath", () => {
    const engine = makeCortexEngineSpy();
    const streams = makeSubscriber();
    createCortexBus({
      engine,
      streams,
      focusedSessionId: () => null,
    });

    nowValue = 0;
    streams.dispatch(envelope("session-X", "running"));

    expect(engine.spawnParticle).not.toHaveBeenCalled();
    expect(engine.setBreath).toHaveBeenCalled();
  });
});
