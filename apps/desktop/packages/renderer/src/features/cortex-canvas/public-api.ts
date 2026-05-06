/*
 * Public Cortex engine facade.
 *
 * Sibling tasks (cortex-bus, agent-visualization, attention-strip) consume
 * only this surface. The engine implementation lives in
 * engine/particle-engine.ts; everything else in this package is internal.
 */

import {
  createParticleEngine,
  type ParticleEngineDebug,
  type ParticleEngineHandle,
  type ParticleEngineOpts,
} from "./engine/particle-engine.js";

export type { BloomGlyph } from "./engine/assembly-bloom.js";
export type { PulseTone } from "./engine/pulse-ring.js";
export type { ParticleSnapshot, EngineSnapshot } from "./engine/particle-engine.js";

export type CreateCortexEngineOpts = ParticleEngineOpts;

export interface CortexEngine extends ParticleEngineHandle {
  __debug: ParticleEngineDebug;
}

export function createCortexEngine(opts: CreateCortexEngineOpts): CortexEngine {
  return createParticleEngine(opts);
}
