import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { createCortexEngine } from "../../packages/renderer/src/features/cortex-canvas/index.js";
import { makeStubCanvas } from "./support/cortex-canvas-stub.js";

const root = globalThis;
const previousMatchMedia = root.matchMedia;
const previousRaf = root.requestAnimationFrame;
const previousCaf = root.cancelAnimationFrame;

let lastRafCb: ((t: number) => void) | null = null;
const rafSpy = vi.fn();

function makeMediaQueryList(matches: boolean): MediaQueryList {
  return {
    addEventListener: vi.fn(),
    addListener: vi.fn(),
    dispatchEvent: vi.fn(() => true),
    matches,
    media: "",
    onchange: null,
    removeEventListener: vi.fn(),
    removeListener: vi.fn(),
  };
}

beforeEach(() => {
  rafSpy.mockReset();
  lastRafCb = null;
  root.matchMedia = () => makeMediaQueryList(false);
  root.requestAnimationFrame = (cb: FrameRequestCallback) => {
    lastRafCb = cb;
    rafSpy(cb);
    return 1;
  };
  root.cancelAnimationFrame = () => undefined;
});

afterEach(() => {
  if (previousMatchMedia) root.matchMedia = previousMatchMedia;
  else Reflect.deleteProperty(root, "matchMedia");
  if (previousRaf) root.requestAnimationFrame = previousRaf;
  else Reflect.deleteProperty(root, "requestAnimationFrame");
  if (previousCaf) root.cancelAnimationFrame = previousCaf;
  else Reflect.deleteProperty(root, "cancelAnimationFrame");
});

describe("cortex engine pause", () => {
  it("pause() halts state mutation; resume() restores it", () => {
    const canvas = makeStubCanvas();
    const engine = createCortexEngine({ canvas, seed: 1 });
    engine.registerLane({ laneId: "lane-a", columnIndex: 0 });
    engine.spawnParticle({ laneId: "lane-a", intensity: 0.5 });

    engine.start();
    expect(rafSpy).toHaveBeenCalledTimes(1);

    // First frame advances state.
    expect(lastRafCb).not.toBeNull();
    lastRafCb!(16);
    const beforePause = engine.__debug.snapshot();
    expect(beforePause.frameCount).toBe(1);

    engine.pause();
    expect(engine.isPaused()).toBe(true);

    // Frame still fires (loop alive) but tick does not mutate state.
    lastRafCb!(32);
    const duringPause = engine.__debug.snapshot();
    expect(duringPause.frameCount).toBe(beforePause.frameCount);
    expect(duringPause.particles[0]?.y).toBe(beforePause.particles[0]?.y);

    engine.resume();
    expect(engine.isPaused()).toBe(false);
    lastRafCb!(48);
    const afterResume = engine.__debug.snapshot();
    expect(afterResume.frameCount).toBe(beforePause.frameCount + 1);

    engine.dispose();
  });
});
