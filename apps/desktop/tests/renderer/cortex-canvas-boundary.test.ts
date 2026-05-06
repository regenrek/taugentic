import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vite-plus/test";

const PACKAGE_ROOT = path.resolve(
  import.meta.dirname,
  "../../packages/renderer/src/features/cortex-canvas",
);

const FORBIDDEN_PATTERNS: RegExp[] = [
  /from\s+["']@\/features\//,
  /from\s+["']\.\.\/features\//,
  /from\s+["']\.\.\/\.\.\/features\//,
  /from\s+["']\.\.\/\.\.\/\.\.\/features\//,
  /from\s+["']@\/lib\/queries/,
  /from\s+["']@\/lib\/ipc/,
  /from\s+["']@taugentic/,
];

/**
 * The cortex-bus is the single sanctioned bridge from cortex-canvas to a
 * sibling feature. It may import from `../streams` (Public API barrel) and
 * nothing else outside cortex-canvas. Every other file must stay inside the
 * package.
 */
const EXTERNAL_IMPORT_ALLOWLIST: ReadonlyArray<{
  file: string;
  externalPrefix: string;
}> = [{ file: "event-bus.ts", externalPrefix: "streams/" }];

const FROM_RE = /from\s+["']([^"']+)["']/g;

function walk(dir: string, acc: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    if (entry === "node_modules" || entry === "__demo__") continue;
    const full = path.join(dir, entry);
    const stats = statSync(full);
    if (stats.isDirectory()) walk(full, acc);
    else if (full.endsWith(".ts") || full.endsWith(".tsx")) acc.push(full);
  }
  return acc;
}

function relativeOutsidePackage(absImport: string): string | null {
  const rel = path.relative(PACKAGE_ROOT, absImport);
  if (rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel))) return null;
  // Anything that resolves outside PACKAGE_ROOT is reported as a path
  // relative to the renderer features root for matching against allowlist.
  const featuresRoot = path.resolve(PACKAGE_ROOT, "..");
  const fromFeatures = path.relative(featuresRoot, absImport);
  return fromFeatures;
}

describe("cortex-canvas boundary", () => {
  it("does not import any features/* sibling, lib/queries, lib/ipc, or @taugentic packages", () => {
    const files = walk(PACKAGE_ROOT);
    expect(files.length).toBeGreaterThan(0);
    const violations: { file: string; line: number; match: string }[] = [];
    for (const file of files) {
      const text = readFileSync(file, "utf8");
      const lines = text.split(/\r?\n/);
      lines.forEach((line, idx) => {
        for (const pattern of FORBIDDEN_PATTERNS) {
          if (pattern.test(line)) {
            violations.push({ file, line: idx + 1, match: line.trim() });
          }
        }
      });
    }
    expect(violations).toEqual([]);
  });

  it("disallows escaping the cortex-canvas package via relative imports, except for the sanctioned bus -> streams bridge", () => {
    const files = walk(PACKAGE_ROOT);
    const violations: { file: string; line: number; match: string }[] = [];
    for (const file of files) {
      const text = readFileSync(file, "utf8");
      const lines = text.split(/\r?\n/);
      const baseName = path.basename(file);
      const allow = EXTERNAL_IMPORT_ALLOWLIST.find((entry) => entry.file === baseName);
      lines.forEach((line, idx) => {
        FROM_RE.lastIndex = 0;
        let m: RegExpExecArray | null = FROM_RE.exec(line);
        while (m !== null) {
          const importPath = m[1]!;
          if (importPath.startsWith(".")) {
            const absImport = path.resolve(path.dirname(file), importPath);
            const escaped = relativeOutsidePackage(absImport);
            if (escaped !== null) {
              const normalized = escaped.replace(/\\/g, "/");
              if (!allow || !normalized.startsWith(allow.externalPrefix)) {
                violations.push({ file, line: idx + 1, match: line.trim() });
              }
            }
          }
          m = FROM_RE.exec(line);
        }
      });
    }
    expect(violations).toEqual([]);
  });
});
