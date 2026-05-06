/*
 * Minimal canvas stub for engine tests in the node test environment.
 *
 * Provides a 2D-context-shaped object whose draw methods are no-ops so the
 * engine's render pipeline can run without jsdom. Tests that need to count
 * draw calls can pass their own context spy and replace getContext.
 */

import { vi } from "vite-plus/test";

export interface StubContext {
  clearRect: ReturnType<typeof vi.fn>;
  save: ReturnType<typeof vi.fn>;
  restore: ReturnType<typeof vi.fn>;
  beginPath: ReturnType<typeof vi.fn>;
  arc: ReturnType<typeof vi.fn>;
  fill: ReturnType<typeof vi.fn>;
  moveTo: ReturnType<typeof vi.fn>;
  lineTo: ReturnType<typeof vi.fn>;
  stroke: ReturnType<typeof vi.fn>;
  fillStyle: string;
  strokeStyle: string;
  globalAlpha: number;
  lineWidth: number;
}

export function makeStubContext(): StubContext {
  return {
    clearRect: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    beginPath: vi.fn(),
    arc: vi.fn(),
    fill: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    stroke: vi.fn(),
    fillStyle: "",
    strokeStyle: "",
    globalAlpha: 1,
    lineWidth: 1,
  };
}

export interface StubCanvas {
  width: number;
  height: number;
  getContext: (kind: string) => StubContext | null;
  ctx: StubContext;
}

export function makeStubCanvas(width = 320, height = 240): HTMLCanvasElement {
  const ctx = makeStubContext();
  const canvas: StubCanvas = {
    width,
    height,
    ctx,
    getContext: (kind: string) => (kind === "2d" ? ctx : null),
  };
  return canvas as unknown as HTMLCanvasElement;
}

export function getStubContext(canvas: HTMLCanvasElement): StubContext {
  return (canvas as unknown as StubCanvas).ctx;
}
