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
  handler: ((e: StreamEvent) => void) | null;
} {
  const slot: {
    handler: ((e: StreamEvent) => void) | null;
    unsub: ReturnType<typeof vi.fn<() => void>>;
  } = {
    handler: null,
    unsub: vi.fn<() => void>(),
  };
  return {
    subscribe(h) {
      slot.handler = h;
      return () => {
        slot.unsub();
      };
    },
    dispatch(e) {
      slot.handler?.(e);
    },
    get unsubscribe() {
      return slot.unsub;
    },
    get handler() {
      return slot.handler;
    },
  };
}

let nowSpy: RestorableSpy | null = null;

beforeEach(() => {
  nowSpy = vi.spyOn(globalThis.performance, "now").mockReturnValue(0);
});

afterEach(() => {
  nowSpy?.mockRestore();
  nowSpy = null;
});

describe("cortex bus dispose", () => {
  it("calls the underlying unsubscribe and silences further engine calls", () => {
    const engine = makeCortexEngineSpy();
    const streams = makeSubscriber();
    const bus = createCortexBus({
      engine,
      streams,
      focusedSessionId: () => "session-A",
    });

    streams.dispatch({
      daemonInstanceId: "d",
      sessionId: "session-A",
      sequence: 1n,
      occurredAtMs: 0n,
      event: { run: { runId: "r-1", status: "running", detail: "" } },
    });
    expect(engine.spawnParticle).toHaveBeenCalledTimes(1);

    bus.dispose();
    expect(streams.unsubscribe).toHaveBeenCalledTimes(1);

    streams.dispatch({
      daemonInstanceId: "d",
      sessionId: "session-A",
      sequence: 2n,
      occurredAtMs: 0n,
      event: { run: { runId: "r-1", status: "running", detail: "" } },
    });
    expect(engine.spawnParticle).toHaveBeenCalledTimes(1);
    expect(engine.setBreath.mock.calls.length).toBeLessThanOrEqual(1);
  });

  it("dispose() is idempotent and never unsubscribes twice", () => {
    const engine = makeCortexEngineSpy();
    const streams = makeSubscriber();
    const bus = createCortexBus({
      engine,
      streams,
      focusedSessionId: () => "session-A",
    });

    bus.dispose();
    bus.dispose();
    bus.dispose();

    expect(streams.unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("swallows underlying unsubscribe errors so consumers can chain teardown safely", () => {
    const engine = makeCortexEngineSpy();
    const subscriber: StreamSubscriber = {
      subscribe() {
        return () => {
          throw new Error("transport gone");
        };
      },
    };
    const bus = createCortexBus({
      engine,
      streams: subscriber,
      focusedSessionId: () => "session-A",
    });

    expect(() => bus.dispose()).not.toThrow();
  });
});
