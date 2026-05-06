/*
 * Typed runtime accessors for the Mission Control --mc-* CSS variables.
 *
 * Reads computed style off the document root and parses sensible numeric
 * fallbacks. Falls back to spec defaults when invoked outside a DOM
 * environment (tests, SSR) so the engine can boot without a window.
 */

export interface MotionTokens {
  particleSpeedPxPerSec: number;
  breathHz: number;
  decayMs: number;
  glowWindowPx: number;
  particleBudget: number;
  fpsCap: number;
  phosphor: string;
  phosphorAmber: string;
  phosphorRed: string;
  grid: string;
  synapse: string;
}

const DEFAULTS: MotionTokens = {
  particleSpeedPxPerSec: 220,
  breathHz: 0.6,
  decayMs: 400,
  glowWindowPx: 80,
  particleBudget: 400,
  fpsCap: 60,
  phosphor: "#9bff9b",
  phosphorAmber: "#f5c451",
  phosphorRed: "#ff6b6b",
  grid: "#1a1a1a",
  synapse: "#2a2a2a",
};

function parseNumeric(raw: string | undefined, fallback: number): number {
  if (!raw) return fallback;
  const trimmed = raw.trim();
  if (trimmed === "") return fallback;
  const numeric = parseFloat(trimmed);
  return Number.isFinite(numeric) ? numeric : fallback;
}

function parseColor(raw: string | undefined, fallback: string): string {
  if (!raw) return fallback;
  const trimmed = raw.trim();
  return trimmed === "" ? fallback : trimmed;
}

function getDocRoot(root?: HTMLElement): HTMLElement | null {
  if (root) return root;
  const doc = (globalThis as { document?: Document }).document;
  return doc?.documentElement ?? null;
}

export function readMotionTokens(root?: HTMLElement): MotionTokens {
  const target = getDocRoot(root);
  const getStyle = (globalThis as { getComputedStyle?: (el: Element) => CSSStyleDeclaration })
    .getComputedStyle;
  if (!target || typeof getStyle !== "function") {
    return { ...DEFAULTS };
  }
  const style = getStyle(target);
  const read = (name: string): string | undefined => style.getPropertyValue(name);
  return {
    particleSpeedPxPerSec: parseNumeric(
      read("--mc-particle-speed"),
      DEFAULTS.particleSpeedPxPerSec,
    ),
    breathHz: parseNumeric(read("--mc-breath-hz"), DEFAULTS.breathHz),
    decayMs: parseNumeric(read("--mc-decay-ms"), DEFAULTS.decayMs),
    glowWindowPx: parseNumeric(read("--mc-glow-window"), DEFAULTS.glowWindowPx),
    particleBudget: parseNumeric(read("--mc-particle-budget"), DEFAULTS.particleBudget),
    fpsCap: parseNumeric(read("--mc-fps-cap"), DEFAULTS.fpsCap),
    phosphor: parseColor(read("--mc-phosphor"), DEFAULTS.phosphor),
    phosphorAmber: parseColor(read("--mc-phosphor-amber"), DEFAULTS.phosphorAmber),
    phosphorRed: parseColor(read("--mc-phosphor-red"), DEFAULTS.phosphorRed),
    grid: parseColor(read("--mc-grid"), DEFAULTS.grid),
    synapse: parseColor(read("--mc-synapse"), DEFAULTS.synapse),
  };
}

export function defaultMotionTokens(): MotionTokens {
  return { ...DEFAULTS };
}
