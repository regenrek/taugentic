import path from "node:path";
import { fileURLToPath } from "node:url";

import { parseArgvFlagValue } from "./argv-flag.mjs";
import { resolveDesktopReleaseProfile } from "./release-profile.mjs";
import {
  assertReleaseTagMatchesPackageVersion,
  readDesktopMainPackageVersion,
} from "./release-version.mjs";

export function isDesktopReleaseTagRef(ref) {
  return typeof ref === "string" && ref.trim().startsWith("refs/tags/v");
}

export function extractDesktopReleaseTag(ref) {
  if (!isDesktopReleaseTagRef(ref)) {
    return null;
  }
  return ref.trim().slice("refs/tags/".length);
}

export function resolveDesktopReleaseRef(argv = [], env = process.env) {
  return parseArgvFlagValue(argv, "--ref") ?? env.GITHUB_REF ?? null;
}

export function resolveDesktopReleasePublishModeForRef(ref, packageVersion, releaseProfile) {
  const tagName = extractDesktopReleaseTag(ref);
  if (tagName === null || releaseProfile !== "stable") {
    return "never";
  }
  assertReleaseTagMatchesPackageVersion(tagName, packageVersion);
  return "always";
}

export async function resolveDesktopReleasePublishMode(argv = [], env = process.env, rootDir) {
  const ref = resolveDesktopReleaseRef(argv, env);
  const releaseProfile = resolveDesktopReleaseProfile(argv, env);
  if (!isDesktopReleaseTagRef(ref)) {
    return "never";
  }
  const packageVersion = await readDesktopMainPackageVersion(rootDir);
  return resolveDesktopReleasePublishModeForRef(ref, packageVersion, releaseProfile);
}

async function main() {
  const publishMode = await resolveDesktopReleasePublishMode(process.argv.slice(2));
  console.log(publishMode);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  await main();
}
