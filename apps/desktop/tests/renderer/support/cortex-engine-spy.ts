import { vi } from "vite-plus/test";

import type { CortexEngine } from "../../../packages/renderer/src/features/cortex-canvas/index.js";

export type CortexEngineSpy = CortexEngine & {
  dispose: ReturnType<typeof vi.fn<CortexEngine["dispose"]>>;
  isPaused: ReturnType<typeof vi.fn<CortexEngine["isPaused"]>>;
  pause: ReturnType<typeof vi.fn<CortexEngine["pause"]>>;
  registerAnchor: ReturnType<typeof vi.fn<CortexEngine["registerAnchor"]>>;
  registerLane: ReturnType<typeof vi.fn<CortexEngine["registerLane"]>>;
  resume: ReturnType<typeof vi.fn<CortexEngine["resume"]>>;
  setBreath: ReturnType<typeof vi.fn<CortexEngine["setBreath"]>>;
  spawnParticle: ReturnType<typeof vi.fn<CortexEngine["spawnParticle"]>>;
  start: ReturnType<typeof vi.fn<CortexEngine["start"]>>;
  stop: ReturnType<typeof vi.fn<CortexEngine["stop"]>>;
  triggerAssemblyBloom: ReturnType<typeof vi.fn<CortexEngine["triggerAssemblyBloom"]>>;
  triggerPulseRing: ReturnType<typeof vi.fn<CortexEngine["triggerPulseRing"]>>;
  unregisterAnchor: ReturnType<typeof vi.fn<CortexEngine["unregisterAnchor"]>>;
  unregisterLane: ReturnType<typeof vi.fn<CortexEngine["unregisterLane"]>>;
};

export interface RestorableSpy {
  mockRestore(): void;
}

export function makeCortexEngineSpy(): CortexEngineSpy {
  return {
    dispose: vi.fn<CortexEngine["dispose"]>(),
    isPaused: vi.fn<CortexEngine["isPaused"]>().mockReturnValue(false),
    pause: vi.fn<CortexEngine["pause"]>(),
    registerAnchor: vi.fn<CortexEngine["registerAnchor"]>(),
    registerLane: vi.fn<CortexEngine["registerLane"]>(),
    resume: vi.fn<CortexEngine["resume"]>(),
    setBreath: vi.fn<CortexEngine["setBreath"]>(),
    spawnParticle: vi.fn<CortexEngine["spawnParticle"]>(),
    start: vi.fn<CortexEngine["start"]>(),
    stop: vi.fn<CortexEngine["stop"]>(),
    triggerAssemblyBloom: vi.fn<CortexEngine["triggerAssemblyBloom"]>(),
    triggerPulseRing: vi.fn<CortexEngine["triggerPulseRing"]>(),
    unregisterAnchor: vi.fn<CortexEngine["unregisterAnchor"]>(),
    unregisterLane: vi.fn<CortexEngine["unregisterLane"]>(),
    __debug: {
      particleCount: () => 0,
      lastFrameMs: () => 0,
      snapshot: () => ({
        timeSec: 0,
        frameCount: 0,
        particles: [],
        blooms: [],
        pulses: [],
        breath: { hz: 0.6, amplitude: 1 },
      }),
      step: () => undefined,
    },
  };
}
