import fs from "node:fs";
import { execFileSync } from "node:child_process";

function git(args) {
  return execFileSync("git", args, { encoding: "utf8" });
}

function getPaths(mode) {
  const args =
    mode === "--tracked"
      ? ["ls-files", "-z"]
      : ["diff", "--cached", "--name-only", "-z"];
  const out = git(args);
  return out.split("\0").filter(Boolean);
}

function loadPatterns() {
  const file = ".forbidden-paths.regex";
  if (!fs.existsSync(file)) throw new Error(`missing ${file}`);
  const raw = fs.readFileSync(file, "utf8");
  const lines = raw
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#"));
  return lines.map((pattern) => new RegExp(pattern));
}

const mode = process.argv[2] ?? "--staged";
if (mode !== "--staged" && mode !== "--tracked") {
  console.error("usage: block-forbidden-staged-files.mjs [--staged|--tracked]");
  process.exit(2);
}

const paths = getPaths(mode);
if (paths.length === 0) process.exit(0);

const patterns = loadPatterns();
const blocked = paths.filter((path) => patterns.some((pattern) => pattern.test(path)));

if (blocked.length === 0) process.exit(0);

const scope = mode === "--tracked" ? "tracked" : "staged";
console.error(`blocked: forbidden file(s) ${scope}:`);
for (const file of blocked) console.error(`- ${file}`);
console.error("");
console.error("fix: unstage/remove the file, or use a safe template path.");
console.error("If this is intentional, update .forbidden-paths.regex.");
process.exit(1);
