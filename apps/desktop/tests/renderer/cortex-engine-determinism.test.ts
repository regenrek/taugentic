import { describe, expect, it } from "vite-plus/test";

import { createCortexEngine } from "../../packages/renderer/src/features/cortex-canvas/index.js";
import { makeStubCanvas } from "./support/cortex-canvas-stub.js";

function scriptedRun(seed: number): unknown {
  const engine = createCortexEngine({ canvas: makeStubCanvas(), seed });
  engine.registerLane({ laneId: "lane-a", columnIndex: 0 });
  engine.registerLane({ laneId: "lane-b", columnIndex: 1 });
  engine.registerAnchor({ anchorId: "anchor-x", x: 100, y: 60 });

  const scriptedSpawns: Array<{ laneId: string; intensity: number }> = [
    { laneId: "lane-a", intensity: 0.2 },
    { laneId: "lane-b", intensity: 0.7 },
    { laneId: "lane-a", intensity: 0.9 },
    { laneId: "lane-a", intensity: 0.5 },
    { laneId: "lane-b", intensity: 0.3 },
  ];

  for (const spawn of scriptedSpawns) {
    engine.spawnParticle(spawn);
    engine.__debug.step(16);
  }
  engine.triggerAssemblyBloom({ laneId: "lane-a", glyph: "ok" });
  engine.triggerPulseRing({ anchorId: "anchor-x", tone: "attention" });
  engine.__debug.step(16);
  engine.__debug.step(16);

  const snap = engine.__debug.snapshot();
  engine.dispose();
  return snap;
}

describe("cortex engine determinism", () => {
  it("produces identical snapshots for identical seed + scripted inputs", () => {
    const a = scriptedRun(42);
    const b = scriptedRun(42);
    expect(JSON.stringify(a)).toEqual(JSON.stringify(b));
  });

  it("differs across different seeds", () => {
    const a = scriptedRun(1);
    const b = scriptedRun(2);
    expect(JSON.stringify(a)).not.toEqual(JSON.stringify(b));
  });
});
