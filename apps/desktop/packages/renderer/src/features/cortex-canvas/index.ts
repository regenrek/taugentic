/*
 * Cortex canvas package barrel.
 *
 * Exports only the public API. Internal engine wiring is intentionally
 * not re-exported -- callers must depend on the contract in public-api.ts.
 */

export { createCortexEngine } from "./public-api.js";
export type {
  CortexEngine,
  CreateCortexEngineOpts,
  EngineSnapshot,
  ParticleSnapshot,
  BloomGlyph,
  PulseTone,
} from "./public-api.js";
export { CortexField, type CortexFieldHandle, type CortexFieldProps } from "./CortexField.js";
export { phosphorDecayClass, phosphorDecayStyle, PHOSPHOR_DECAY_CLASS } from "./phosphor-decay.js";
