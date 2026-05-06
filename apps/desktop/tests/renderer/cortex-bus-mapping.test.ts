import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import {
  createCortexBus,
  type CortexBusOptions,
} from "../../packages/renderer/src/features/cortex-canvas/event-bus.js";
import type { CortexEngine } from "../../packages/renderer/src/features/cortex-canvas/index.js";
import type {
  StreamEvent,
  StreamSubscriber,
} from "../../packages/renderer/src/features/streams/index.js";
import type { RestorableSpy } from "./support/cortex-engine-spy.js";

type EngineSpy = CortexEngine & {
  spawnParticle: ReturnType<typeof vi.fn<CortexEngine["spawnParticle"]>>;
  triggerPulseRing: ReturnType<typeof vi.fn<CortexEngine["triggerPulseRing"]>>;
  triggerAssemblyBloom: ReturnType<typeof vi.fn<CortexEngine["triggerAssemblyBloom"]>>;
  setBreath: ReturnType<typeof vi.fn<CortexEngine["setBreath"]>>;
  registerLane: ReturnType<typeof vi.fn<CortexEngine["registerLane"]>>;
  unregisterLane: ReturnType<typeof vi.fn<CortexEngine["unregisterLane"]>>;
  registerAnchor: ReturnType<typeof vi.fn<CortexEngine["registerAnchor"]>>;
  unregisterAnchor: ReturnType<typeof vi.fn<CortexEngine["unregisterAnchor"]>>;
  start: ReturnType<typeof vi.fn<CortexEngine["start"]>>;
  stop: ReturnType<typeof vi.fn<CortexEngine["stop"]>>;
  pause: ReturnType<typeof vi.fn<CortexEngine["pause"]>>;
  resume: ReturnType<typeof vi.fn<CortexEngine["resume"]>>;
  isPaused: ReturnType<typeof vi.fn<CortexEngine["isPaused"]>>;
  dispose: ReturnType<typeof vi.fn<CortexEngine["dispose"]>>;
};

function makeEngineSpy(): EngineSpy {
  return {
    start: vi.fn<CortexEngine["start"]>(),
    stop: vi.fn<CortexEngine["stop"]>(),
    pause: vi.fn<CortexEngine["pause"]>(),
    resume: vi.fn<CortexEngine["resume"]>(),
    isPaused: vi.fn<CortexEngine["isPaused"]>().mockReturnValue(false),
    dispose: vi.fn<CortexEngine["dispose"]>(),
    spawnParticle: vi.fn<CortexEngine["spawnParticle"]>(),
    triggerPulseRing: vi.fn<CortexEngine["triggerPulseRing"]>(),
    triggerAssemblyBloom: vi.fn<CortexEngine["triggerAssemblyBloom"]>(),
    setBreath: vi.fn<CortexEngine["setBreath"]>(),
    registerLane: vi.fn<CortexEngine["registerLane"]>(),
    unregisterLane: vi.fn<CortexEngine["unregisterLane"]>(),
    registerAnchor: vi.fn<CortexEngine["registerAnchor"]>(),
    unregisterAnchor: vi.fn<CortexEngine["unregisterAnchor"]>(),
    __debug: {
      particleCount: () => 0,
      lastFrameMs: () => 0,
      snapshot: () => ({
        timeSec: 0,
        frameCount: 0,
        particles: [],
        blooms: [],
        pulses: [],
        breath: { hz: 0.6, amplitude: 1 },
      }),
      step: () => undefined,
    },
  };
}

interface FakeSubscriber extends StreamSubscriber {
  dispatch(event: StreamEvent): void;
  unsubscribe: ReturnType<typeof vi.fn<() => void>>;
  handler: ((event: StreamEvent) => void) | null;
}

function makeSubscriber(): FakeSubscriber {
  const fake: FakeSubscriber = {
    handler: null,
    unsubscribe: vi.fn<() => void>(),
    subscribe(handler) {
      fake.handler = handler;
      return () => {
        fake.unsubscribe();
      };
    },
    dispatch(event) {
      fake.handler?.(event);
    },
  };
  return fake;
}

function makeBus(overrides: Partial<CortexBusOptions> = {}): {
  bus: ReturnType<typeof createCortexBus>;
  engine: EngineSpy;
  streams: FakeSubscriber;
} {
  const engine = (overrides.engine as EngineSpy | undefined) ?? makeEngineSpy();
  const streams = (overrides.streams as FakeSubscriber | undefined) ?? makeSubscriber();
  const focusedSessionId = overrides.focusedSessionId ?? (() => "session-focused");
  const onLaneEffect = overrides.onLaneEffect;
  const bus = createCortexBus({ engine, streams, focusedSessionId, onLaneEffect });
  return { bus, engine, streams };
}

function envelope(sessionId: string, event: StreamEvent["event"]): StreamEvent {
  return {
    daemonInstanceId: "daemon-1",
    sessionId,
    sequence: 1n,
    occurredAtMs: 0n,
    event,
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

function advanceTo(ms: number): void {
  nowValue = ms;
}

describe("cortex bus event mapping", () => {
  it("translates each envelope to the engine call dictated by the spec", () => {
    const onLaneEffect = vi.fn();
    const { engine, streams } = makeBus({
      focusedSessionId: () => "session-focused",
      onLaneEffect,
    });

    advanceTo(0);
    streams.dispatch(
      envelope("session-focused", { run: { runId: "r-1", status: "running", detail: "" } }),
    );
    advanceTo(10);
    streams.dispatch(
      envelope("session-focused", {
        run: { runId: "r-1", status: "waitingForApproval", detail: "" },
      }),
    );
    advanceTo(20);
    streams.dispatch(
      envelope("session-focused", {
        approval: {
          phase: "requested",
          request: {
            expiresAtMs: 60_000n,
            id: "a-1",
            requestedAtMs: 0n,
            runId: "r-1",
            scope: "processExec",
            target: { kind: "processExec", command: "echo ok" },
            reason: "needs human",
          },
        },
      }),
    );
    advanceTo(30);
    streams.dispatch(
      envelope("session-focused", {
        artifact: {
          artifact: {
            id: "ar-1",
            runId: "r-1",
            kind: "Patch",
            storagePath: "/tmp/patch",
          },
        },
      }),
    );
    advanceTo(40);
    streams.dispatch(
      envelope("session-focused", {
        artifact: {
          artifact: {
            id: "ar-2",
            runId: "r-1",
            kind: "CommandLog",
            storagePath: "/tmp/cmd",
          },
        },
      }),
    );
    advanceTo(50);
    streams.dispatch(
      envelope("session-focused", { run: { runId: "r-1", status: "completed", detail: "" } }),
    );
    advanceTo(60);
    streams.dispatch(
      envelope("session-focused", { run: { runId: "r-1", status: "failed", detail: "boom" } }),
    );
    advanceTo(70);
    streams.dispatch(
      envelope("session-focused", {
        session: { sessionId: "session-focused", status: "completed" },
      }),
    );
    advanceTo(80);
    streams.dispatch(
      envelope("session-focused", { session: { sessionId: "session-focused", status: "failed" } }),
    );

    expect(engine.spawnParticle.mock.calls.map((c) => c[0])).toEqual([
      { laneId: "session-focused", intensity: 1 },
    ]);

    expect(engine.triggerPulseRing.mock.calls.map((c) => c[0])).toEqual([
      { anchorId: "session-focused", tone: "attention" },
      { anchorId: "session-focused", tone: "attention" },
      { anchorId: "session-focused", tone: "failed" },
      { anchorId: "session-focused", tone: "failed" },
    ]);

    expect(engine.triggerAssemblyBloom.mock.calls.map((c) => c[0])).toEqual([
      { laneId: "session-focused", glyph: "diff" },
      { laneId: "session-focused", glyph: "tool" },
      { laneId: "session-focused", glyph: "ok" },
      { laneId: "session-focused", glyph: "ok" },
    ]);

    expect(onLaneEffect.mock.calls.map((c) => c[0])).toEqual([
      { laneId: "session-focused", effect: "sweep" },
      { laneId: "session-focused", effect: "sweep" },
      { laneId: "session-focused", effect: "failed-border" },
      { laneId: "session-focused", effect: "failed-border" },
    ]);
  });

  it("session-running events spawn a particle on the focused lane", () => {
    const { engine, streams } = makeBus({ focusedSessionId: () => "session-focused" });
    streams.dispatch(
      envelope("session-focused", { session: { sessionId: "session-focused", status: "running" } }),
    );
    expect(engine.spawnParticle).toHaveBeenCalledWith({
      laneId: "session-focused",
      intensity: 1,
    });
  });
});
