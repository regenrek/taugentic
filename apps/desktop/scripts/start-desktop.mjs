import { execFile, spawn } from "node:child_process";
import { constants } from "node:fs";
import { access } from "node:fs/promises";
import { resolve } from "node:path";
import { platform } from "node:process";
import { promisify } from "node:util";

import {
  macosDevelopmentAppLaunch,
  materializeMacosDevelopmentApp,
  resolveBunExecutable,
} from "./macos-development-app.mjs";

const desktopRoot = resolve(import.meta.dirname, "..");
const repositoryRoot = resolve(desktopRoot, "../..");
const hot = process.argv.includes("--hot");
const release = process.argv.includes("--release");

if (hot && release) {
  throw new Error("Hot reload and release mode are mutually exclusive");
}

const profile = release ? "release" : "debug";

const { stdout } = await promisify(execFile)(
  "cargo",
  ["metadata", "--format-version=1", "--no-deps"],
  { cwd: repositoryRoot },
);
const metadata = JSON.parse(stdout);
const daemonName = platform === "win32" ? "ta-daemon.exe" : "ta-daemon";
const daemonBinary = resolve(metadata.target_directory, profile, daemonName);

async function prepareDesktopArtifacts() {
  const cargoProfileArguments = profile === "release" ? ["--release"] : [];
  await promisify(execFile)(
    "cargo",
    [
      "build",
      "--package",
      "ta-orchestrator",
      "--bin",
      "ta-daemon",
      ...cargoProfileArguments,
    ],
    { cwd: repositoryRoot },
  );
  await promisify(execFile)(
    "cargo",
    [
      "build",
      "--package",
      "ta-desktop-native",
      "--lib",
      ...cargoProfileArguments,
    ],
    { cwd: repositoryRoot },
  );
  await promisify(execFile)(
    "pnpm",
    ["--filter", "@taugentic/desktop-daemon-native", "stage-native"],
    { cwd: desktopRoot, env: { ...process.env, CARGO_PROFILE: profile } },
  );
  await access(daemonBinary, constants.X_OK);
}

await prepareDesktopArtifacts();

if (!process.argv.includes("--check")) {
  const developmentApp = await materializeMacosDevelopmentApp({
    desktopRoot,
    executableSourcePath: await resolveBunExecutable(),
    entrypointPath: resolve(desktopRoot, "src/main.tsx"),
  });
  const launch = macosDevelopmentAppLaunch({
    developmentApp,
    desktopRoot,
    daemonBinary,
    daemonSocketName: process.env.TAUGENTIC_DAEMON_SOCKET_NAME,
    hot,
  });
  const desktop = spawn(launch.command, launch.arguments, {
    cwd: desktopRoot,
    stdio: "inherit",
  });

  const [code] = await new Promise((resolveExit, reject) => {
    desktop.once("error", reject);
    desktop.once("exit", (exitCode) => resolveExit([exitCode]));
  });
  process.exitCode = code ?? 1;
}
