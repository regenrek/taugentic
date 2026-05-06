import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { createCortexEngine } from "../../packages/renderer/src/features/cortex-canvas/index.js";
import { getStubContext, makeStubCanvas } from "./support/cortex-canvas-stub.js";

const root = globalThis;
const previousMatchMedia = root.matchMedia;
const previousRaf = root.requestAnimationFrame;
const previousCaf = root.cancelAnimationFrame;
const rafSpy = vi.fn();

function makeMediaQueryList(matches: boolean, media: string): MediaQueryList {
  return {
    addEventListener: vi.fn(),
    addListener: vi.fn(),
    dispatchEvent: vi.fn(() => true),
    matches,
    media,
    onchange: null,
    removeEventListener: vi.fn(),
    removeListener: vi.fn(),
  };
}

beforeEach(() => {
  rafSpy.mockReset();
  root.matchMedia = (q: string) => makeMediaQueryList(q === "(prefers-reduced-motion: reduce)", q);
  root.requestAnimationFrame = (cb: FrameRequestCallback) => {
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

describe("cortex engine reduced motion", () => {
  it("never schedules rAF and performs a single static draw", () => {
    const canvas = makeStubCanvas();
    const ctx = getStubContext(canvas);
    const engine = createCortexEngine({ canvas, seed: 1 });

    engine.start();

    expect(rafSpy).not.toHaveBeenCalled();
    expect(ctx.clearRect).toHaveBeenCalledTimes(1);

    engine.dispose();
  });
});
