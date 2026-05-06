import path from "node:path";
import { fileURLToPath } from "node:url";

import { normalizeStageTargetPlatform, parseArgvFlagValue } from "./package-layout.mjs";

const REQUIRED_MAC_SIGNING_ENV = [
  "APPLE_APP_SPECIFIC_PASSWORD",
  "APPLE_ID",
  "APPLE_TEAM_ID",
  "CSC_KEY_PASSWORD",
  "CSC_LINK",
];

const REQUIRED_WINDOWS_SIGNING_ENV = ["CSC_KEY_PASSWORD", "CSC_LINK"];

export function requiredReleaseSigningEnv(platform) {
  if (platform === "darwin") {
    return REQUIRED_MAC_SIGNING_ENV;
  }
  if (platform === "win32") {
    return REQUIRED_WINDOWS_SIGNING_ENV;
  }
  return [];
}

export function missingReleaseSigningEnv(platform, env) {
  return requiredReleaseSigningEnv(platform).filter((name) => {
    const value = env[name];
    return typeof value !== "string" || value.trim().length === 0;
  });
}

export function assertReleaseSigningEnv(platform, env) {
  const missing = missingReleaseSigningEnv(platform, env);
  if (missing.length === 0) {
    return;
  }
  throw new Error(`missing release signing env for ${platform}: ${missing.join(", ")}`);
}

function resolveReleasePlatform(argv, env, fallbackPlatform) {
  const rawPlatform =
    parseArgvFlagValue(argv, "--platform") ?? env.TAUGENTIC_DESKTOP_RELEASE_PLATFORM;
  return normalizeStageTargetPlatform(rawPlatform ?? fallbackPlatform, fallbackPlatform);
}

function shouldRequireSigning(argv, env) {
  return argv.includes("--require-signing") || env.TAUGENTIC_DESKTOP_REQUIRE_SIGNING === "1";
}

function main() {
  const argv = process.argv.slice(2);
  const platform = resolveReleasePlatform(argv, process.env, process.platform);
  if (!shouldRequireSigning(argv, process.env)) {
    console.log(`release signing disabled for ${platform}`);
    return;
  }
  assertReleaseSigningEnv(platform, process.env);
  console.log(`release signing env ready for ${platform}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  main();
}
