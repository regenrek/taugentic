import fs from "node:fs";
import { execFileSync } from "node:child_process";

function git(args) {
  return execFileSync("git", args, { encoding: "utf8" });
}

function getStagedPaths() {
  const out = git(["diff", "--cached", "--name-only", "-z"]);
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

const staged = getStagedPaths();
if (staged.length === 0) process.exit(0);

const patterns = loadPatterns();
const blocked = staged.filter((path) => patterns.some((pattern) => pattern.test(path)));

if (blocked.length === 0) process.exit(0);

console.error("blocked: forbidden file(s) staged:");
for (const file of blocked) console.error(`- ${file}`);
console.error("");
console.error("fix: unstage/remove the file, or use a safe template path.");
console.error("If this is intentional, update .forbidden-paths.regex.");
process.exit(1);
