import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parseArgvFlagValue } from "./argv-flag.mjs";
import { desktopRootDir } from "./package-layout.mjs";

export function normalizeReleaseTagVersion(tagName) {
  if (typeof tagName !== "string") {
    return null;
  }
  const trimmed = tagName.trim();
  if (trimmed.length === 0) {
    return null;
  }
  return trimmed.startsWith("v") ? trimmed.slice(1) : trimmed;
}

export async function readDesktopMainPackageVersion(rootDir = desktopRootDir) {
  const manifestPath = path.join(rootDir, "packages", "main", "package.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  return String(manifest.version ?? "");
}

export function assertReleaseTagMatchesPackageVersion(tagName, packageVersion) {
  const tagVersion = normalizeReleaseTagVersion(tagName);
  if (tagVersion === null) {
    return;
  }
  if (tagVersion !== packageVersion) {
    throw new Error(
      `release tag ${tagName} does not match desktop package version ${packageVersion}`,
    );
  }
}

async function main() {
  const argv = process.argv.slice(2);
  const tagName =
    parseArgvFlagValue(argv, "--tag") ??
    process.env.GITHUB_REF_NAME ??
    process.env.TAUGENTIC_DESKTOP_RELEASE_TAG;
  const packageVersion = await readDesktopMainPackageVersion();
  assertReleaseTagMatchesPackageVersion(tagName, packageVersion);
  console.log(`desktop release version ready: ${packageVersion}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  await main();
}
