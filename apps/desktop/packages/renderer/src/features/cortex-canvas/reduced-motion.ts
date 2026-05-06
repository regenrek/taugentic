/*
 * Mission Control reduced-motion gate.
 *
 * Used by the engine to no-op rAF and by CortexField to render a single
 * static breath snapshot. Tolerates missing matchMedia (node tests).
 */

const REDUCED_QUERY = "(prefers-reduced-motion: reduce)";

type MQL = {
  matches: boolean;
  addEventListener?: (type: "change", cb: (e: { matches: boolean }) => void) => void;
  removeEventListener?: (type: "change", cb: (e: { matches: boolean }) => void) => void;
  addListener?: (cb: (e: { matches: boolean }) => void) => void;
  removeListener?: (cb: (e: { matches: boolean }) => void) => void;
};

type MatchMediaFn = (q: string) => MQL;

function getMatchMedia(): MatchMediaFn | null {
  const direct = (globalThis as { matchMedia?: MatchMediaFn }).matchMedia;
  if (typeof direct === "function") return direct;
  const win = (globalThis as { window?: { matchMedia?: MatchMediaFn } }).window;
  if (win && typeof win.matchMedia === "function") return win.matchMedia;
  return null;
}

export function prefersReducedMotion(): boolean {
  const mm = getMatchMedia();
  if (!mm) return false;
  try {
    return mm(REDUCED_QUERY).matches === true;
  } catch {
    return false;
  }
}
