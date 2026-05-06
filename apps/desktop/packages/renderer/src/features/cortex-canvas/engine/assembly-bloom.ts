/*
 * Assembly-Bloom primitive.
 *
 * A short-lived cluster that forms, holds, and decays at a lane anchor.
 * Lifetime is the global decay token. The engine owns the bloom array;
 * this module only provides the update + render functions.
 */

export type BloomGlyph = "ok" | "tool" | "diff";

export interface BloomState {
  laneId: string;
  bornAtSec: number;
  glyph?: BloomGlyph;
}

export interface BloomFrameOpts {
  /** Seconds since engine start. */
  timeSec: number;
  /** Decay window in seconds (derived from --mc-decay-ms). */
  lifetimeSec: number;
  /** Where the lane sits horizontally in canvas pixels. */
  laneX: number;
  /** Where the lane anchor sits vertically in canvas pixels. */
  laneY: number;
  color: string;
}

export function bloomProgress(b: BloomState, timeSec: number, lifetimeSec: number): number {
  if (lifetimeSec <= 0) return 1;
  return Math.max(0, Math.min(1, (timeSec - b.bornAtSec) / lifetimeSec));
}

export function isBloomDead(b: BloomState, timeSec: number, lifetimeSec: number): boolean {
  return bloomProgress(b, timeSec, lifetimeSec) >= 1;
}

export function renderBloom(
  ctx: CanvasRenderingContext2D | null | undefined,
  b: BloomState,
  opts: BloomFrameOpts,
): void {
  if (!ctx) return;
  const t = bloomProgress(b, opts.timeSec, opts.lifetimeSec);
  if (t >= 1) return;
  // Form -> hold -> decay arc.
  const form = Math.min(1, t * 3);
  const decay = Math.max(0, 1 - Math.max(0, t - 0.66) * 3);
  const radius = 4 + form * 8;
  const alpha = Math.max(0, Math.min(1, decay));
  ctx.save();
  ctx.fillStyle = opts.color;
  ctx.globalAlpha = alpha * 0.55;
  ctx.beginPath();
  ctx.arc(opts.laneX, opts.laneY, radius, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}
