import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import {
  LANE_FAILED_BORDER_CLASS,
  LANE_SWEEP_CLASS,
  LANE_SWEEP_DURATION_MS,
  __resetLaneEffectRegistry,
  applyLaneEffect,
  clearFailedBorder,
  hasFailedBorder,
  registerLaneRow,
} from "../../packages/renderer/src/features/agent-visualization/lane-effects/index.js";

interface FakeClassList {
  add(name: string): void;
  remove(name: string): void;
  contains(name: string): boolean;
}

interface FakeElement {
  classList: FakeClassList;
  offsetWidth: number;
}

function makeFakeElement(): FakeElement {
  const classes = new Set<string>();
  return {
    offsetWidth: 0,
    classList: {
      add(name: string) {
        classes.add(name);
      },
      remove(name: string) {
        classes.delete(name);
      },
      contains(name: string): boolean {
        return classes.has(name);
      },
    },
  };
}

beforeEach(() => {
  __resetLaneEffectRegistry();
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  __resetLaneEffectRegistry();
});

describe("lane-effects registry", () => {
  it("registerLaneRow stores the element and returns an unregister handle", () => {
    const el = makeFakeElement();
    const handle = registerLaneRow("session-A", el as unknown as HTMLElement);

    applyLaneEffect({ laneId: "session-A", effect: "sweep" });
    expect(el.classList.contains(LANE_SWEEP_CLASS)).toBe(true);

    handle.unregister();
    const fresh = makeFakeElement();
    applyLaneEffect({ laneId: "session-A", effect: "sweep" });
    expect(fresh.classList.contains(LANE_SWEEP_CLASS)).toBe(false);
  });

  it("sweep adds the sweep class then removes it after LANE_SWEEP_DURATION_MS", () => {
    const el = makeFakeElement();
    registerLaneRow("session-B", el as unknown as HTMLElement);

    applyLaneEffect({ laneId: "session-B", effect: "sweep" });
    expect(el.classList.contains(LANE_SWEEP_CLASS)).toBe(true);

    vi.advanceTimersByTime(LANE_SWEEP_DURATION_MS - 1);
    expect(el.classList.contains(LANE_SWEEP_CLASS)).toBe(true);

    vi.advanceTimersByTime(2);
    expect(el.classList.contains(LANE_SWEEP_CLASS)).toBe(false);
  });

  it("failed-border applies the persistent class and survives clearFailedBorder", () => {
    const el = makeFakeElement();
    registerLaneRow("session-C", el as unknown as HTMLElement);

    applyLaneEffect({ laneId: "session-C", effect: "failed-border" });
    expect(el.classList.contains(LANE_FAILED_BORDER_CLASS)).toBe(true);
    expect(hasFailedBorder("session-C")).toBe(true);

    vi.advanceTimersByTime(LANE_SWEEP_DURATION_MS * 4);
    expect(el.classList.contains(LANE_FAILED_BORDER_CLASS)).toBe(true);

    clearFailedBorder("session-C");
    expect(el.classList.contains(LANE_FAILED_BORDER_CLASS)).toBe(false);
    expect(hasFailedBorder("session-C")).toBe(false);
  });

  it("registering after a failed-border applies the failed class to the new element", () => {
    applyLaneEffect({ laneId: "session-D", effect: "failed-border" });
    expect(hasFailedBorder("session-D")).toBe(true);

    const el = makeFakeElement();
    registerLaneRow("session-D", el as unknown as HTMLElement);
    expect(el.classList.contains(LANE_FAILED_BORDER_CLASS)).toBe(true);
  });

  it("does not throw or apply classes when no element is registered", () => {
    expect(() => applyLaneEffect({ laneId: "session-unknown", effect: "sweep" })).not.toThrow();
    expect(() =>
      applyLaneEffect({ laneId: "session-unknown", effect: "failed-border" }),
    ).not.toThrow();
    expect(hasFailedBorder("session-unknown")).toBe(true);
  });

  it("repeated sweeps cancel the previous timer and still cleanup once", () => {
    const el = makeFakeElement();
    registerLaneRow("session-E", el as unknown as HTMLElement);

    applyLaneEffect({ laneId: "session-E", effect: "sweep" });
    vi.advanceTimersByTime(100);
    applyLaneEffect({ laneId: "session-E", effect: "sweep" });
    expect(el.classList.contains(LANE_SWEEP_CLASS)).toBe(true);

    vi.advanceTimersByTime(LANE_SWEEP_DURATION_MS - 1);
    expect(el.classList.contains(LANE_SWEEP_CLASS)).toBe(true);

    vi.advanceTimersByTime(2);
    expect(el.classList.contains(LANE_SWEEP_CLASS)).toBe(false);
  });
});
