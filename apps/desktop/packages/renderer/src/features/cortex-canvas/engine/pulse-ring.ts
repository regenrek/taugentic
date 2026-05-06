/*
 * Pulse-Ring primitive.
 *
 * Concentric expanding rings emitted from anchor points to mark attention
 * or failure events. Lifetime is the global decay window times two.
 */

export type PulseTone = "attention" | "failed";

export interface PulseRingState {
  anchorId: string;
  bornAtSec: number;
  tone: PulseTone;
}

export interface PulseFrameOpts {
  timeSec: number;
  /** Decay window in seconds (derived from --mc-decay-ms). */
  lifetimeSec: number;
  anchorX: number;
  anchorY: number;
  attentionColor: string;
  failedColor: string;
  maxRadiusPx: number;
}

function pulseProgress(p: PulseRingState, timeSec: number, lifetimeSec: number): number {
  if (lifetimeSec <= 0) return 1;
  return Math.max(0, Math.min(1, (timeSec - p.bornAtSec) / (lifetimeSec * 2)));
}

export function isPulseDead(p: PulseRingState, timeSec: number, lifetimeSec: number): boolean {
  return pulseProgress(p, timeSec, lifetimeSec) >= 1;
}

export function renderPulseRing(
  ctx: CanvasRenderingContext2D | null | undefined,
  p: PulseRingState,
  opts: PulseFrameOpts,
): void {
  if (!ctx) return;
  const t = pulseProgress(p, opts.timeSec, opts.lifetimeSec);
  if (t >= 1) return;
  const radius = 2 + t * opts.maxRadiusPx;
  const alpha = Math.max(0, 1 - t);
  ctx.save();
  ctx.strokeStyle = p.tone === "failed" ? opts.failedColor : opts.attentionColor;
  ctx.lineWidth = 1.2;
  ctx.globalAlpha = alpha;
  ctx.beginPath();
  ctx.arc(opts.anchorX, opts.anchorY, radius, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
}
