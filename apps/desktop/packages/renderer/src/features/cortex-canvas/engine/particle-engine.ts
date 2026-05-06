/*
 * Mission Control particle engine.
 *
 * Owns the single rAF loop and the particle pool. Subsystems
 * (dot-grid, synapse, assembly-bloom, pulse-ring) are pure render +
 * update modules invoked from this file's frame function -- there is
 * intentionally no second loop anywhere in the package.
 *
 * Determinism: a mulberry32 PRNG is seeded from `opts.seed` and used for
 * every randomized choice (idle breath wobble, particle jitter, glyph
 * fallback). Same seed + same input sequence => identical snapshots.
 *
 * Performance budgets are hard:
 *   - fps cap: skip work if a frame consumes more than the budget.
 *   - particle budget: FIFO prune the oldest when the pool exceeds it.
 *   - tab hidden: pause rAF and resume on visibilitychange.
 */

import { defaultMotionTokens, readMotionTokens, type MotionTokens } from "../motion-tokens.js";
import { prefersReducedMotion } from "../reduced-motion.js";
import { renderDotGrid } from "./dot-grid.js";
import {
  bloomProgress,
  isBloomDead,
  renderBloom,
  type BloomGlyph,
  type BloomState,
} from "./assembly-bloom.js";
import { isPulseDead, renderPulseRing, type PulseRingState, type PulseTone } from "./pulse-ring.js";
import { renderSynapse, type SynapseParticleView } from "./synapse.js";

export interface ParticleEngineOpts {
  canvas: HTMLCanvasElement;
  seed?: number;
  fpsCap?: number;
  particleBudget?: number;
}

export interface SpawnParticleArgs {
  laneId: string;
  intensity?: number;
}

export interface PulseRingArgs {
  anchorId: string;
  tone: PulseTone;
}

export interface AssemblyBloomArgs {
  laneId: string;
  glyph?: BloomGlyph;
}

export interface BreathArgs {
  hz?: number;
  amplitude?: number;
}

export interface RegisterLaneArgs {
  laneId: string;
  columnIndex: number;
}

export interface UnregisterLaneArgs {
  laneId: string;
}

export interface RegisterAnchorArgs {
  anchorId: string;
  x: number;
  y: number;
}

export interface UnregisterAnchorArgs {
  anchorId: string;
}

export interface ParticleEngineDebug {
  particleCount(): number;
  lastFrameMs(): number;
  snapshot(): EngineSnapshot;
  /** Drive a single deterministic frame (test-only; bypasses rAF). */
  step(deltaMs: number): void;
}

export interface ParticleEngineHandle {
  start(): void;
  stop(): void;
  pause(): void;
  resume(): void;
  isPaused(): boolean;
  dispose(): void;
  spawnParticle(args: SpawnParticleArgs): void;
  triggerPulseRing(args: PulseRingArgs): void;
  triggerAssemblyBloom(args: AssemblyBloomArgs): void;
  setBreath(args: BreathArgs): void;
  registerLane(args: RegisterLaneArgs): void;
  unregisterLane(args: UnregisterLaneArgs): void;
  registerAnchor(args: RegisterAnchorArgs): void;
  unregisterAnchor(args: UnregisterAnchorArgs): void;
  __debug: ParticleEngineDebug;
}

export interface ParticleSnapshot {
  id: number;
  laneId: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  intensity: number;
  bornAtSec: number;
  color: string;
}

export interface EngineSnapshot {
  timeSec: number;
  frameCount: number;
  particles: ParticleSnapshot[];
  blooms: { laneId: string; bornAtSec: number; glyph?: BloomGlyph }[];
  pulses: { anchorId: string; bornAtSec: number; tone: PulseTone }[];
  breath: { hz: number; amplitude: number };
}

interface Particle {
  id: number;
  laneId: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  intensity: number;
  bornAtSec: number;
  color: string;
}

interface LaneEntry {
  laneId: string;
  columnIndex: number;
}

interface AnchorEntry {
  anchorId: string;
  x: number;
  y: number;
}

const COLUMN_STRIDE_PX = 100;
const COLUMN_OFFSET_PX = 50;
const Y_INITIAL_PX = 0;
const PULSE_MAX_RADIUS_PX = 64;

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function getRaf(): ((cb: (t: number) => void) => number) | null {
  const fn = (globalThis as { requestAnimationFrame?: (cb: (t: number) => void) => number })
    .requestAnimationFrame;
  return typeof fn === "function" ? fn : null;
}

function getCaf(): ((id: number) => void) | null {
  const fn = (globalThis as { cancelAnimationFrame?: (id: number) => void }).cancelAnimationFrame;
  return typeof fn === "function" ? fn : null;
}

function getNow(): () => number {
  const perf = (globalThis as { performance?: { now(): number } }).performance;
  if (perf && typeof perf.now === "function") return () => perf.now();
  return () => Date.now();
}

function getDocument(): Document | null {
  return (globalThis as { document?: Document }).document ?? null;
}

export function createParticleEngine(opts: ParticleEngineOpts): ParticleEngineHandle {
  const tokens: MotionTokens = readMotionTokensSafe();
  const fpsCap = opts.fpsCap ?? tokens.fpsCap;
  const particleBudget = opts.particleBudget ?? tokens.particleBudget;
  const frameBudgetMs = fpsCap > 0 ? 1000 / fpsCap : 16.6667;
  const seed = opts.seed ?? 1;

  const canvas = opts.canvas;
  const ctx = safeGetCtx(canvas);

  const prng = mulberry32(seed);

  const state = {
    running: false,
    paused: false,
    disposed: false,
    timeSec: 0,
    frameCount: 0,
    lastFrameMs: 0,
    particleSeq: 0,
    breath: { hz: tokens.breathHz, amplitude: 1 },
    particles: [] as Particle[],
    blooms: [] as BloomState[],
    pulses: [] as PulseRingState[],
    lanes: new Map<string, LaneEntry>(),
    anchors: new Map<string, AnchorEntry>(),
    rafId: 0 as number,
    nowFn: getNow(),
    lastFrameTime: 0,
  };

  const decayLifetimeSec = tokens.decayMs / 1000;

  function readMotionTokensSafe(): MotionTokens {
    try {
      return readMotionTokens();
    } catch {
      return defaultMotionTokens();
    }
  }

  function safeGetCtx(c: HTMLCanvasElement): CanvasRenderingContext2D | null {
    try {
      return c.getContext?.("2d") as CanvasRenderingContext2D | null;
    } catch {
      return null;
    }
  }

  function laneX(laneId: string): number {
    const lane = state.lanes.get(laneId);
    const idx = lane?.columnIndex ?? 0;
    return COLUMN_OFFSET_PX + idx * COLUMN_STRIDE_PX;
  }

  function colorForIntensity(intensity: number): string {
    if (intensity >= 0.85) return tokens.phosphorRed;
    if (intensity >= 0.55) return tokens.phosphorAmber;
    return tokens.phosphor;
  }

  function pruneBudget(): void {
    const overflow = state.particles.length - particleBudget;
    if (overflow > 0) state.particles.splice(0, overflow);
  }

  function spawnParticle(args: SpawnParticleArgs): void {
    if (state.disposed) return;
    const intensity = clamp01(args.intensity ?? 0.4);
    const baseSpeed = tokens.particleSpeedPxPerSec;
    const jitter = (prng() - 0.5) * 12;
    const verticalJitter = 0.6 + prng() * 0.8;
    const id = ++state.particleSeq;
    const particle: Particle = {
      id,
      laneId: args.laneId,
      x: laneX(args.laneId) + jitter,
      y: Y_INITIAL_PX,
      vx: jitter * 0.05,
      vy: baseSpeed * verticalJitter * (0.5 + intensity),
      intensity,
      bornAtSec: state.timeSec,
      color: colorForIntensity(intensity),
    };
    state.particles.push(particle);
    pruneBudget();
  }

  function triggerPulseRing(args: PulseRingArgs): void {
    if (state.disposed) return;
    state.pulses.push({
      anchorId: args.anchorId,
      bornAtSec: state.timeSec,
      tone: args.tone,
    });
  }

  function triggerAssemblyBloom(args: AssemblyBloomArgs): void {
    if (state.disposed) return;
    state.blooms.push({
      laneId: args.laneId,
      bornAtSec: state.timeSec,
      glyph: args.glyph,
    });
  }

  function setBreath(args: BreathArgs): void {
    if (args.hz != null && Number.isFinite(args.hz)) state.breath.hz = args.hz;
    if (args.amplitude != null && Number.isFinite(args.amplitude)) {
      state.breath.amplitude = clamp01(args.amplitude);
    }
  }

  function registerLane(args: RegisterLaneArgs): void {
    state.lanes.set(args.laneId, { laneId: args.laneId, columnIndex: args.columnIndex });
  }

  function unregisterLane(args: UnregisterLaneArgs): void {
    state.lanes.delete(args.laneId);
  }

  function registerAnchor(args: RegisterAnchorArgs): void {
    state.anchors.set(args.anchorId, { anchorId: args.anchorId, x: args.x, y: args.y });
  }

  function unregisterAnchor(args: UnregisterAnchorArgs): void {
    state.anchors.delete(args.anchorId);
  }

  function tick(deltaSec: number): void {
    if (state.paused) return;
    state.timeSec += deltaSec;
    state.frameCount += 1;
    // Advance particles. Drop offscreen (logical y > 2000) early.
    const live: Particle[] = [];
    for (const p of state.particles) {
      const ny = p.y + p.vy * deltaSec;
      const nx = p.x + p.vx * deltaSec;
      if (ny > 2000) continue;
      p.x = nx;
      p.y = ny;
      live.push(p);
    }
    state.particles = live;
    pruneBudget();
    state.blooms = state.blooms.filter((b) => !isBloomDead(b, state.timeSec, decayLifetimeSec));
    state.pulses = state.pulses.filter((p) => !isPulseDead(p, state.timeSec, decayLifetimeSec));
  }

  function draw(): void {
    if (!ctx) return;
    const width = canvas.width || 0;
    const height = canvas.height || 0;
    if (width <= 0 || height <= 0) return;
    if (typeof ctx.clearRect === "function") ctx.clearRect(0, 0, width, height);
    renderDotGrid(ctx, {
      width,
      height,
      cellSize: 24,
      color: tokens.grid,
      breathHz: state.breath.hz,
      breathAmplitude: state.breath.amplitude,
      timeSec: state.timeSec,
    });
    const synapseView: SynapseParticleView[] = state.particles.map((p) => ({
      laneId: p.laneId,
      x: p.x,
      y: p.y,
      intensity: p.intensity,
    }));
    renderSynapse(ctx, synapseView, {
      glowWindowPx: tokens.glowWindowPx,
      color: tokens.synapse,
    });
    if (typeof ctx.beginPath === "function") {
      ctx.save();
      for (const p of state.particles) {
        ctx.fillStyle = p.color;
        ctx.globalAlpha = Math.max(0.2, Math.min(1, p.intensity + 0.2));
        ctx.beginPath();
        ctx.arc(p.x, p.y, 1.4 + p.intensity * 1.4, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.restore();
    }
    for (const b of state.blooms) {
      renderBloom(ctx, b, {
        timeSec: state.timeSec,
        lifetimeSec: decayLifetimeSec,
        laneX: laneX(b.laneId),
        laneY: height * 0.5,
        color: tokens.phosphor,
      });
    }
    for (const p of state.pulses) {
      const anchor = state.anchors.get(p.anchorId);
      const ax = anchor?.x ?? width * 0.5;
      const ay = anchor?.y ?? height * 0.5;
      renderPulseRing(ctx, p, {
        timeSec: state.timeSec,
        lifetimeSec: decayLifetimeSec,
        anchorX: ax,
        anchorY: ay,
        attentionColor: tokens.phosphorAmber,
        failedColor: tokens.phosphorRed,
        maxRadiusPx: PULSE_MAX_RADIUS_PX,
      });
    }
  }

  function frame(timeMs: number): void {
    if (!state.running || state.disposed) return;
    if (isTabHidden()) {
      // Skip mutation while hidden, do not request another frame.
      return;
    }
    const deltaMs = state.lastFrameTime === 0 ? 16 : timeMs - state.lastFrameTime;
    state.lastFrameTime = timeMs;
    const deltaSec = Math.max(0, deltaMs) / 1000;
    const startedAt = state.nowFn();
    tick(deltaSec);
    if (deltaMs <= frameBudgetMs * 1.5) {
      draw();
    }
    state.lastFrameMs = state.nowFn() - startedAt;
    schedule();
  }

  function schedule(): void {
    if (!state.running || state.disposed) return;
    const raf = getRaf();
    if (!raf) return;
    state.rafId = raf(frame);
  }

  function isTabHidden(): boolean {
    const doc = getDocument();
    return doc?.hidden === true;
  }

  function staticDraw(): void {
    draw();
  }

  function start(): void {
    if (state.disposed || state.running) return;
    if (prefersReducedMotion()) {
      // Reduced-motion: never schedule rAF, draw a single static frame.
      state.running = false;
      staticDraw();
      return;
    }
    state.running = true;
    state.lastFrameTime = 0;
    schedule();
  }

  function stop(): void {
    state.running = false;
    if (state.rafId) {
      const caf = getCaf();
      caf?.(state.rafId);
      state.rafId = 0;
    }
  }

  function pause(): void {
    state.paused = true;
  }

  function resume(): void {
    state.paused = false;
  }

  function isPaused(): boolean {
    return state.paused;
  }

  const onVisibilityChange = (): void => {
    if (state.disposed) return;
    if (!isTabHidden() && state.running) {
      state.lastFrameTime = 0;
      schedule();
    }
  };

  const doc = getDocument();
  doc?.addEventListener?.("visibilitychange", onVisibilityChange);

  function dispose(): void {
    if (state.disposed) return;
    state.disposed = true;
    stop();
    doc?.removeEventListener?.("visibilitychange", onVisibilityChange);
    state.particles = [];
    state.blooms = [];
    state.pulses = [];
    state.lanes.clear();
    state.anchors.clear();
  }

  function snapshot(): EngineSnapshot {
    return {
      timeSec: round(state.timeSec, 6),
      frameCount: state.frameCount,
      particles: state.particles.map((p) => ({
        id: p.id,
        laneId: p.laneId,
        x: round(p.x, 4),
        y: round(p.y, 4),
        vx: round(p.vx, 4),
        vy: round(p.vy, 4),
        intensity: round(p.intensity, 4),
        bornAtSec: round(p.bornAtSec, 6),
        color: p.color,
      })),
      blooms: state.blooms.map((b) => ({
        laneId: b.laneId,
        bornAtSec: round(b.bornAtSec, 6),
        glyph: b.glyph,
      })),
      pulses: state.pulses.map((p) => ({
        anchorId: p.anchorId,
        bornAtSec: round(p.bornAtSec, 6),
        tone: p.tone,
      })),
      breath: { hz: state.breath.hz, amplitude: state.breath.amplitude },
    };
  }

  function debugStep(deltaMs: number): void {
    if (state.disposed) return;
    const safe = Math.max(0, deltaMs);
    tick(safe / 1000);
    draw();
  }

  // Mark progress; consumers can also read the bloom helper directly.
  void bloomProgress;

  return {
    start,
    stop,
    pause,
    resume,
    isPaused,
    dispose,
    spawnParticle,
    triggerPulseRing,
    triggerAssemblyBloom,
    setBreath,
    registerLane,
    unregisterLane,
    registerAnchor,
    unregisterAnchor,
    __debug: {
      particleCount: () => state.particles.length,
      lastFrameMs: () => state.lastFrameMs,
      snapshot,
      step: debugStep,
    },
  };
}

function clamp01(n: number): number {
  if (!Number.isFinite(n)) return 0;
  if (n < 0) return 0;
  if (n > 1) return 1;
  return n;
}

function round(n: number, digits: number): number {
  const f = Math.pow(10, digits);
  return Math.round(n * f) / f;
}
