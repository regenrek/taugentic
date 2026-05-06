/*
 * Synapse primitive.
 *
 * Renders short line-glow connections between particles that share a lane
 * and fall inside the glow window. Stateless: the engine passes the live
 * particle slice; this module does no allocation across frames.
 */

export interface SynapseParticleView {
  laneId: string;
  x: number;
  y: number;
  intensity: number;
}

export interface SynapseOpts {
  glowWindowPx: number;
  color: string;
}

export function renderSynapse(
  ctx: CanvasRenderingContext2D | null | undefined,
  particles: ReadonlyArray<SynapseParticleView>,
  opts: SynapseOpts,
): void {
  if (!ctx || particles.length < 2) return;
  const { glowWindowPx, color } = opts;
  const window = Math.max(1, glowWindowPx);
  ctx.save();
  ctx.strokeStyle = color;
  ctx.lineWidth = 0.6;
  for (let i = 0; i < particles.length; i++) {
    const a = particles[i]!;
    for (let j = i + 1; j < particles.length; j++) {
      const b = particles[j]!;
      if (a.laneId !== b.laneId) continue;
      const dx = a.x - b.x;
      const dy = a.y - b.y;
      const dist = Math.sqrt(dx * dx + dy * dy);
      if (dist > window) continue;
      const alpha = Math.max(0, 1 - dist / window) * Math.min(1, (a.intensity + b.intensity) * 0.5);
      if (alpha <= 0.01) continue;
      ctx.globalAlpha = alpha;
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.stroke();
    }
  }
  ctx.restore();
}
