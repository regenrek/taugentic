/*
 * Dot-Field-Breath primitive.
 *
 * Pure render module. The engine calls renderDotGrid once per frame.
 * Breath amplitude is sin-driven by the global breath clock; no own loop.
 */

export interface DotGridOpts {
  width: number;
  height: number;
  cellSize: number;
  color: string;
  breathHz: number;
  breathAmplitude: number;
  /** Seconds since engine start (deterministic). */
  timeSec: number;
}

export function renderDotGrid(
  ctx: CanvasRenderingContext2D | null | undefined,
  opts: DotGridOpts,
): void {
  if (!ctx) return;
  const { width, height, cellSize, color, breathHz, breathAmplitude, timeSec } = opts;
  if (cellSize <= 0 || width <= 0 || height <= 0) return;
  const baseAlpha = 0.72;
  const breath = Math.sin(timeSec * breathHz * Math.PI * 2);
  const alpha = Math.max(0, Math.min(1, baseAlpha + breath * breathAmplitude * 0.28));
  const radius = Math.max(1.1, cellSize * 0.09);
  ctx.save();
  ctx.fillStyle = color;
  ctx.globalAlpha = alpha;
  for (let x = cellSize / 2; x < width; x += cellSize) {
    for (let y = cellSize / 2; y < height; y += cellSize) {
      ctx.beginPath();
      ctx.arc(x, y, radius, 0, Math.PI * 2);
      ctx.fill();
    }
  }
  ctx.restore();
}
