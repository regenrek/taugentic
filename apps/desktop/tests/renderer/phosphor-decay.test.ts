import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vite-plus/test";

import {
  PHOSPHOR_DECAY_CLASS,
  phosphorDecayClass,
  phosphorDecayStyle,
} from "../../packages/renderer/src/features/cortex-canvas/index.js";

const GLOBAL_CSS_PATH = path.resolve(
  import.meta.dirname,
  "../../packages/renderer/src/styles/global.css",
);

describe("phosphor-decay helper", () => {
  it("returns the canonical class name", () => {
    expect(phosphorDecayClass()).toBe("mc-phosphor-decay");
    expect(phosphorDecayClass()).toBe(PHOSPHOR_DECAY_CLASS);
  });

  it("matches a CSS rule shipped in global.css", () => {
    const css = readFileSync(GLOBAL_CSS_PATH, "utf8");
    expect(css).toMatch(/\.mc-phosphor-decay\s*\{/);
    expect(css).toMatch(/@keyframes\s+mc-phosphor-decay\b/);
  });

  it("composes an empty style when no override is provided", () => {
    expect(phosphorDecayStyle()).toEqual({});
    expect(phosphorDecayStyle({})).toEqual({});
  });

  it("emits a --mc-decay-ms override when ms is provided", () => {
    const style = phosphorDecayStyle({ ms: 250 }) as Record<string, string>;
    expect(style["--mc-decay-ms"]).toBe("250ms");
  });

  it("clamps negative durations to 0ms", () => {
    const style = phosphorDecayStyle({ ms: -5 }) as Record<string, string>;
    expect(style["--mc-decay-ms"]).toBe("0ms");
  });
});
