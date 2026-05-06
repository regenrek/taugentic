import path from "node:path";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { parseArgvFlagValue } from "./argv-flag.mjs";
import {
  getDesktopReleaseProfileConfig,
  resolveDesktopReleaseProfile,
} from "./release-profile.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));

export const desktopRootDir = path.resolve(__dirname, "..");
export const repoRootDir = path.resolve(desktopRootDir, "../..");
export const artifactRootDir = path.join(desktopRootDir, ".artifacts");
export const stagedAppDir = path.join(artifactRootDir, "package-app");
export const stagedResourcesDir = path.join(artifactRootDir, "resources");
export const stagedResourcesBinDir = path.join(stagedResourcesDir, "bin");
export const stagedAppMainEntry = "packages/main/dist/index.js";

/**
 * @param {string} raw
 * @param {import("node:os").Platform} fallbackPlatform
 * @returns {import("node:os").Platform}
 */
export function normalizeStageTargetPlatform(raw, fallbackPlatform) {
  if (typeof raw !== "string") {
    return fallbackPlatform;
  }
  const s = raw.trim().toLowerCase();
  if (s === "darwin" || s === "macos") {
    return "darwin";
  }
  if (s === "linux") {
    return "linux";
  }
  if (s === "win32" || s === "windows" || s === "win") {
    return "win32";
  }
  return fallbackPlatform;
}

/**
 * Staging target OS (darwin | linux | win32). Precedence: --platform=, TAUGENTIC_DESKTOP_PACKAGE_PLATFORM, npm_config_platform, fallback.
 * @param {string[]} argv
 * @param {NodeJS.ProcessEnv} env
 * @param {import("node:os").Platform} fallbackPlatform
 */
export function resolveStageTargetPlatform(argv, env, fallbackPlatform) {
  const fromArg = parseArgvFlagValue(argv, "--platform");
  if (fromArg) {
    return normalizeStageTargetPlatform(fromArg, fallbackPlatform);
  }
  const fromEnvVar = env.TAUGENTIC_DESKTOP_PACKAGE_PLATFORM?.trim();
  if (fromEnvVar) {
    return normalizeStageTargetPlatform(fromEnvVar, fallbackPlatform);
  }
  const fromNpm = env.npm_config_platform?.trim();
  if (fromNpm) {
    return normalizeStageTargetPlatform(fromNpm, fallbackPlatform);
  }
  return fallbackPlatform;
}

/**
 * Optional cargo `--target` triple for cross-compiled daemon artifacts.
 * @param {string[]} argv
 * @param {NodeJS.ProcessEnv} env
 * @returns {string | null}
 */
export function resolveStageCargoTargetTriple(argv, env) {
  const fromArg = parseArgvFlagValue(argv, "--cargo-target");
  if (fromArg) {
    return fromArg;
  }
  const fromEnv = env.TAUGENTIC_DESKTOP_CARGO_TARGET?.trim();
  return fromEnv && fromEnv.length > 0 ? fromEnv : null;
}

/**
 * Cross-platform packaging requires an explicit cargo target triple so the daemon
 * is built and staged from the matching target directory.
 * @param {import("node:os").Platform} targetPlatform
 * @param {import("node:os").Platform} hostPlatform
 * @param {string | null} cargoTargetTriple
 */
export function assertStageTargetPlatformConfiguration(
  targetPlatform,
  hostPlatform,
  cargoTargetTriple,
) {
  if (targetPlatform === hostPlatform || cargoTargetTriple) {
    return;
  }
  throw new Error(
    `cross-platform desktop packaging from ${hostPlatform} to ${targetPlatform} requires --cargo-target or TAUGENTIC_DESKTOP_CARGO_TARGET`,
  );
}

export function daemonBinaryFileNameForPlatform(platform) {
  return platform === "win32" ? "ta-daemon.exe" : "ta-daemon";
}

export function buildStageAppPackageManifest({ version, releaseProfile = "stable" }) {
  const releaseConfig = getDesktopReleaseProfileConfig(releaseProfile);
  return {
    name: releaseConfig.packageName,
    author: "Taugentic",
    description: `${releaseConfig.productName} desktop runtime shell`,
    private: true,
    type: "module",
    version,
    main: stagedAppMainEntry,
    dependencies: {
      "@taugentic/desktop-shared": "file:./packages/shared",
    },
  };
}

export { parseArgvFlagValue, getDesktopReleaseProfileConfig, resolveDesktopReleaseProfile };

export function stageCopyPlan(targetAppDir = stagedAppDir) {
  return [
    {
      from: path.join(desktopRootDir, "packages", "main", "dist"),
      to: path.join(targetAppDir, "packages", "main", "dist"),
    },
    {
      from: path.join(desktopRootDir, "packages", "preload", "dist"),
      to: path.join(targetAppDir, "packages", "preload", "dist"),
    },
    {
      from: path.join(desktopRootDir, "packages", "renderer", "dist"),
      to: path.join(targetAppDir, "packages", "renderer", "dist"),
    },
    {
      from: path.join(desktopRootDir, "packages", "shared", "dist"),
      to: path.join(targetAppDir, "packages", "shared", "dist"),
    },
    {
      from: path.join(desktopRootDir, "packages", "shared", "package.json"),
      to: path.join(targetAppDir, "packages", "shared", "package.json"),
    },
  ];
}
