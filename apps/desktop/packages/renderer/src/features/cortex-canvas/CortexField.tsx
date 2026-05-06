/*
 * Cortex field React mount.
 *
 * Owns a single <canvas> sized via ResizeObserver. Creates the engine
 * once, exposes it through an imperative handle, and forwards pause and
 * unmount intent. No domain wiring lives here; the cortex-bus task
 * connects features/streams to the imperative handle later.
 */

import {
  forwardRef,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  type CSSProperties,
} from "react";

import { createCortexEngine, type CortexEngine } from "./public-api.js";
import { prefersReducedMotion } from "./reduced-motion.js";

export interface CortexFieldProps {
  paused?: boolean;
  seed?: number;
  fpsCap?: number;
  particleBudget?: number;
  className?: string;
  style?: CSSProperties;
}

export interface CortexFieldHandle {
  engine: CortexEngine | null;
}

export const CortexField = forwardRef<CortexFieldHandle, CortexFieldProps>(function CortexField(
  { paused, seed, fpsCap, particleBudget, className, style },
  ref,
) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const engineRef = useRef<CortexEngine | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);

  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return undefined;
    const engine = createCortexEngine({
      canvas,
      seed,
      fpsCap,
      particleBudget,
    });
    engineRef.current = engine;

    const container = containerRef.current;
    const dpr = (globalThis as { devicePixelRatio?: number }).devicePixelRatio ?? 1;
    const sizeTo = (width: number, height: number): void => {
      const w = Math.max(1, Math.floor(width * dpr));
      const h = Math.max(1, Math.floor(height * dpr));
      if (canvas.width !== w) canvas.width = w;
      if (canvas.height !== h) canvas.height = h;
    };
    if (container) sizeTo(container.clientWidth, container.clientHeight);

    let observer: ResizeObserver | null = null;
    const RO = (globalThis as { ResizeObserver?: typeof ResizeObserver }).ResizeObserver;
    if (container && typeof RO === "function") {
      observer = new RO((entries) => {
        for (const entry of entries) {
          const { width, height } = entry.contentRect;
          sizeTo(width, height);
        }
      });
      observer.observe(container);
    }

    if (prefersReducedMotion()) {
      // Single static draw via start(); rAF will not be scheduled.
      engine.start();
    } else {
      engine.start();
    }

    return () => {
      observer?.disconnect();
      engine.dispose();
      engineRef.current = null;
    };
    // Engine is created once per mount; prop changes are pushed via the
    // paused effect below and the imperative handle. Reading initial
    // seed/fpsCap/particleBudget by ref is intentional, hence empty deps.
  }, []);

  useLayoutEffect(() => {
    const engine = engineRef.current;
    if (!engine) return;
    if (paused) engine.pause();
    else engine.resume();
  }, [paused]);

  useImperativeHandle(
    ref,
    () => ({
      get engine(): CortexEngine | null {
        return engineRef.current;
      },
    }),
    [],
  );

  return (
    <div
      ref={containerRef}
      className={className}
      style={{ position: "relative", inlineSize: "100%", blockSize: "100%", ...style }}
      data-cortex-field=""
    >
      <canvas
        ref={canvasRef}
        style={{
          inlineSize: "100%",
          blockSize: "100%",
          display: "block",
          pointerEvents: "none",
        }}
      />
    </div>
  );
});
