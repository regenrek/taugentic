import { execFile, spawn } from "node:child_process";
import { readFile, rm } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

import {
  macosDevelopmentAppLaunch,
  materializeMacosDevelopmentApp,
} from "./macos-development-app.mjs";

const desktopRoot = resolve(import.meta.dirname, "..");
const repositoryRoot = resolve(desktopRoot, "../..");

export function parseVoiceAcceptanceMetadata(source) {
  const value = JSON.parse(source);
  if (
    value === null ||
    typeof value !== "object" ||
    value.version !== 1 ||
    value.permission !== "authorized" ||
    !Number.isInteger(value.captured_frames) ||
    value.captured_frames < 1 ||
    !Number.isInteger(value.completed_playback_tickets) ||
    value.completed_playback_tickets < 1 ||
    value.terminal !== "interrupted" ||
    value.teardown !== true
  ) {
    throw new Error(
      "Voice acceptance metadata does not satisfy the fixed schema",
    );
  }
  return value;
}

async function releaseBinaryPath() {
  const { stdout } = await promisify(execFile)(
    "cargo",
    ["metadata", "--format-version=1", "--no-deps"],
    {
      cwd: repositoryRoot,
    },
  );
  return join(
    JSON.parse(stdout).target_directory,
    "release",
    "voice-hardware-acceptance",
  );
}

async function main() {
  await promisify(execFile)(
    "cargo",
    [
      "build",
      "-p",
      "ta-macos-avfoundation",
      "--bin",
      "voice-hardware-acceptance",
      "--features",
      "hardware-acceptance",
      "--release",
    ],
    { cwd: repositoryRoot },
  );
  const resultPath = join(
    desktopRoot,
    ".taugentic-development",
    `voice-acceptance-${randomUUID()}.json`,
  );
  try {
    const developmentApp = await materializeMacosDevelopmentApp({
      desktopRoot,
      executableSourcePath: await releaseBinaryPath(),
    });
    const launch = macosDevelopmentAppLaunch({
      developmentApp,
      desktopRoot,
      hot: false,
      applicationArguments: ["--voice-acceptance-result", resultPath],
    });
    const app = spawn(launch.command, launch.arguments, {
      cwd: desktopRoot,
      stdio: "inherit",
    });
    const [exitCode] = await new Promise((resolveExit, reject) => {
      app.once("error", reject);
      app.once("exit", (code) => resolveExit([code]));
    });
    if (exitCode !== 0)
      throw new Error("Voice acceptance application did not complete");
    parseVoiceAcceptanceMetadata(await readFile(resultPath, "utf8"));
  } finally {
    await rm(resultPath, { force: true });
  }
}

if (import.meta.main) await main();
