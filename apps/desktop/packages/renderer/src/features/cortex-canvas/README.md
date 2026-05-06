# cortex-canvas

Domain-free Mission Control visualization engine. Owns one `<canvas>` and
one `requestAnimationFrame` loop. Sibling slices wire domain state in via
the public API; this package never imports any other `features/*`.

## Public API

Import only from the package barrel (`./index.ts`) or `./public-api.ts`:

```ts
import { createCortexEngine, CortexField } from "@/features/cortex-canvas";
```

Surfaces:

- `createCortexEngine({ canvas, seed?, fpsCap?, particleBudget? })` -> `CortexEngine`
- `CortexField` React component (forwardRef, exposes `engine` via handle)
- `phosphorDecayClass()`, `phosphorDecayStyle({ ms? })` for DOM consumers
- `prefersReducedMotion()`, `onReducedMotionChange(cb)`
- `readMotionTokens(root?)`, `defaultMotionTokens()`

`CortexEngine` methods (consumed by `t-lr4l-cortex-bus`):

- Lifecycle: `start`, `stop`, `pause`, `resume`, `isPaused`, `dispose`
- Input: `spawnParticle({ laneId, intensity? })`,
  `triggerPulseRing({ anchorId, tone })`,
  `triggerAssemblyBloom({ laneId, glyph? })`,
  `setBreath({ hz?, amplitude? })`
- Layout: `registerLane({ laneId, columnIndex })`,
  `unregisterLane({ laneId })`,
  `registerAnchor({ anchorId, x, y })`
- `__debug.particleCount() | lastFrameMs() | snapshot() | step(deltaMs)`

## Hard rules

- No `features/*` imports inside this package (boundary test enforces it).
- Single `<canvas>` mounted by `CortexField`. Engine never touches the DOM
  outside that canvas.
- Single rAF loop owned by `engine/particle-engine.ts`. Subsystems
  (`dot-grid`, `synapse`, `assembly-bloom`, `pulse-ring`) are pure render
  modules invoked from the loop, never independent loops.
- Performance budgets: `fpsCap` defaults to 60, `particleBudget` to 400
  with FIFO prune of the oldest. Tab hidden -> rAF stops; visibilitychange
  reschedules.
- Determinism: same seed + same input sequence -> identical
  `__debug.snapshot()`. Mulberry32 PRNG inline; no deps.
- Reduced motion: `start()` performs a single static draw and never
  schedules rAF.

## Manual visual demo

Storybook is intentionally not installed. For manual smoke testing, mount
`CortexField` in a dev-only route and feed it via the imperative handle:

```tsx
const ref = useRef<CortexFieldHandle>(null);
useLayoutEffect(() => {
  ref.current?.engine?.registerLane({ laneId: "demo", columnIndex: 0 });
  const id = setInterval(() => {
    ref.current?.engine?.spawnParticle({ laneId: "demo", intensity: Math.random() });
  }, 200);
  return () => clearInterval(id);
}, []);
```

Bus integration (`t-lr4l-cortex-bus`) follows this exact shape, replacing
the `setInterval` with the streams subscription.
