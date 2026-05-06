/*
 * Lane-effects registry.
 *
 * Tiny pub/sub bridge that lets the cortex bus (in features/cortex-canvas)
 * apply DOM-level visual effects to lane row elements that live in the
 * SessionRail (features/overview). The bus emits opaque `onLaneEffect`
 * callbacks; the visualization panel forwards them here, and SessionRailItem
 * registers its DOM node via a ref callback.
 *
 * No React deps. Reduced-motion is honored by the underlying CSS rules
 * (see styles/global.css: .mc-lane-sweep / .mc-lane-failed-border).
 */

export type LaneEffect = "sweep" | "failed-border";

export const LANE_SWEEP_CLASS = "mc-lane-sweep";
export const LANE_FAILED_BORDER_CLASS = "mc-lane-failed-border";
export const LANE_SWEEP_DURATION_MS = 320;

const rows = new Map<string, HTMLElement>();
const sweepTimers = new Map<string, ReturnType<typeof setTimeout>>();
const failedBorders = new Set<string>();

export interface RegisterLaneRowResult {
  unregister(): void;
}

export function registerLaneRow(laneId: string, el: HTMLElement | null): RegisterLaneRowResult {
  if (el === null) {
    rows.delete(laneId);
    return { unregister: noop };
  }
  rows.set(laneId, el);
  if (failedBorders.has(laneId)) {
    el.classList.add(LANE_FAILED_BORDER_CLASS);
  }
  return {
    unregister(): void {
      const current = rows.get(laneId);
      if (current === el) {
        rows.delete(laneId);
      }
    },
  };
}

export function applyLaneEffect(args: { laneId: string; effect: LaneEffect }): void {
  const { laneId, effect } = args;
  if (effect === "sweep") {
    applySweep(laneId);
    return;
  }
  if (effect === "failed-border") {
    applyFailedBorder(laneId);
    return;
  }
}

export function clearFailedBorder(laneId: string): void {
  failedBorders.delete(laneId);
  const el = rows.get(laneId);
  el?.classList.remove(LANE_FAILED_BORDER_CLASS);
}

export function hasFailedBorder(laneId: string): boolean {
  return failedBorders.has(laneId);
}

/** Test-only: drops every registered row, sweep timer, and failed flag. */
export function __resetLaneEffectRegistry(): void {
  for (const timer of sweepTimers.values()) {
    clearTimeout(timer);
  }
  sweepTimers.clear();
  rows.clear();
  failedBorders.clear();
}

function applySweep(laneId: string): void {
  const el = rows.get(laneId);
  if (el !== undefined) {
    el.classList.remove(LANE_SWEEP_CLASS);
    // Restart the keyframe animation deterministically by forcing reflow.
    void el.offsetWidth;
    el.classList.add(LANE_SWEEP_CLASS);
  }
  const existing = sweepTimers.get(laneId);
  if (existing !== undefined) {
    clearTimeout(existing);
  }
  const timer = setTimeout(() => {
    const current = rows.get(laneId);
    current?.classList.remove(LANE_SWEEP_CLASS);
    sweepTimers.delete(laneId);
  }, LANE_SWEEP_DURATION_MS);
  sweepTimers.set(laneId, timer);
}

function applyFailedBorder(laneId: string): void {
  failedBorders.add(laneId);
  const el = rows.get(laneId);
  el?.classList.add(LANE_FAILED_BORDER_CLASS);
}

function noop(): void {}
