import { describe, expect, it } from "vite-plus/test";

import { createCortexEngine } from "../../packages/renderer/src/features/cortex-canvas/index.js";
import { makeStubCanvas } from "./support/cortex-canvas-stub.js";

describe("cortex engine particle budget", () => {
  it("FIFO-prunes the oldest when the budget is exceeded", () => {
    const canvas = makeStubCanvas();
    const engine = createCortexEngine({ canvas, seed: 1 });
    engine.registerLane({ laneId: "lane-a", columnIndex: 0 });

    for (let i = 0; i < 401; i++) {
      engine.spawnParticle({ laneId: "lane-a", intensity: 0.4 });
    }

    expect(engine.__debug.particleCount()).toBe(400);
    const snapshot = engine.__debug.snapshot();
    // The very first particle (id === 1) must have been dropped; the surviving
    // window is the most-recent 400 (ids 2..401).
    const ids = snapshot.particles.map((p) => p.id);
    expect(ids[0]).toBe(2);
    expect(ids[ids.length - 1]).toBe(401);
    engine.dispose();
  });

  it("respects a custom particleBudget", () => {
    const canvas = makeStubCanvas();
    const engine = createCortexEngine({ canvas, seed: 1, particleBudget: 8 });
    engine.registerLane({ laneId: "lane-a", columnIndex: 0 });
    for (let i = 0; i < 12; i++) {
      engine.spawnParticle({ laneId: "lane-a" });
    }
    expect(engine.__debug.particleCount()).toBe(8);
    engine.dispose();
  });
});
