import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { createCortexBus } from "../../packages/renderer/src/features/cortex-canvas/event-bus.js";
import type {
  StreamEvent,
  StreamSubscriber,
} from "../../packages/renderer/src/features/streams/index.js";
import { makeCortexEngineSpy, type RestorableSpy } from "./support/cortex-engine-spy.js";

function makeSubscriber(): StreamSubscriber & {
  dispatch(e: StreamEvent): void;
} {
  let handler: ((e: StreamEvent) => void) | null = null;
  return {
    subscribe(h) {
      handler = h;
      return () => undefined;
    },
    dispatch(e) {
      handler?.(e);
    },
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

describe("cortex bus throttle", () => {
  it("throttles setBreath to at most ~4x per second under flood", () => {
    const engine = makeCortexEngineSpy();
    const streams = makeSubscriber();
    createCortexBus({
      engine,
      streams,
      focusedSessionId: () => "session-A",
    });

    const event: StreamEvent = {
      daemonInstanceId: "d",
      sessionId: "session-A",
      sequence: 1n,
      occurredAtMs: 0n,
      event: { run: { runId: "r-1", status: "running", detail: "" } },
    };

    // Dispatch 100 events spread across 1000ms (one every 10ms).
    for (let i = 0; i < 100; i += 1) {
      nowValue = i * 10;
      streams.dispatch(event);
    }

    const calls = engine.setBreath.mock.calls.length;
    // First call at t=0 plus one per 250ms boundary (250, 500, 750) => up to 4.
    expect(calls).toBeLessThanOrEqual(5);
    expect(calls).toBeGreaterThanOrEqual(3);
  });

  it("breath frequency rises with throughput (saturating at 0.6 + 1.4 Hz)", () => {
    const engine = makeCortexEngineSpy();
    const streams = makeSubscriber();
    createCortexBus({
      engine,
      streams,
      focusedSessionId: () => "session-A",
    });

    const event: StreamEvent = {
      daemonInstanceId: "d",
      sessionId: "session-A",
      sequence: 1n,
      occurredAtMs: 0n,
      event: { run: { runId: "r-1", status: "running", detail: "" } },
    };

    // First call: t=0, only 1 event in window.
    nowValue = 0;
    streams.dispatch(event);
    expect(engine.setBreath.mock.calls.length).toBeGreaterThan(0);
    const firstCall = engine.setBreath.mock.calls[0]!;
    expect(firstCall[0].hz).toBeCloseTo(0.6 + 0.05, 5);

    // Pile 50 events between t=0 and t=300ms; only the call after the
    // 250ms throttle boundary should fire.
    for (let i = 1; i < 50; i += 1) {
      nowValue = (i / 50) * 300;
      streams.dispatch(event);
    }

    const allCalls = engine.setBreath.mock.calls;
    expect(allCalls.length).toBeGreaterThan(0);
    const lastCall = allCalls[allCalls.length - 1]!;
    const lastHz = lastCall[0].hz;
    // 50 events in <1s window -> 50 * 0.05 = 2.5, but capped at delta 1.4.
    expect(lastHz).toBeCloseTo(0.6 + 1.4, 5);
  });
});
