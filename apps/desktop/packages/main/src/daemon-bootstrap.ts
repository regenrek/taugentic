import { spawn } from "node:child_process";

import type { DaemonControlStatusResult } from "@taugentic/desktop-shared";
import { parseDaemonControlStatusResult } from "@taugentic/desktop-shared/validation";
import {
  assertDesktopBootstrapLaunchSpecExists,
  createDesktopDaemonBootstrapConfig,
  type DesktopDaemonBootstrapLaunchSpec,
  resolveDesktopDaemonBootstrapLaunchSpec,
  resolveDesktopRepoRoot,
} from "./desktop-bootstrap-config.js";
import { app } from "./electron.js";

const DAEMON_BOOTSTRAP_START_COMMAND = "start";
const DAEMON_BOOTSTRAP_SNAPSHOT_COMMAND = "snapshot";
const DAEMON_BOOTSTRAP_RECONCILE_COMMAND = "reconcile";
const DAEMON_BOOTSTRAP_ENABLE_BACKGROUND_COMMAND = "enable-background";
const DAEMON_BOOTSTRAP_DISABLE_BACKGROUND_COMMAND = "disable-background";
const DAEMON_BOOTSTRAP_STOP_COMMAND = "stop";
type DaemonBootstrapCommandName =
  | typeof DAEMON_BOOTSTRAP_START_COMMAND
  | typeof DAEMON_BOOTSTRAP_SNAPSHOT_COMMAND
  | typeof DAEMON_BOOTSTRAP_RECONCILE_COMMAND
  | typeof DAEMON_BOOTSTRAP_ENABLE_BACKGROUND_COMMAND
  | typeof DAEMON_BOOTSTRAP_DISABLE_BACKGROUND_COMMAND
  | typeof DAEMON_BOOTSTRAP_STOP_COMMAND;
const repoRoot = resolveDesktopRepoRoot(import.meta.url);

function resolveDaemonBootstrapLaunchSpecForCommand(
  options: {
    cwd: string;
    env: NodeJS.ProcessEnv;
    isPackaged: boolean;
    platform: NodeJS.Platform;
    repoRoot: string;
    resourcesPath: string;
  },
  commandName: DaemonBootstrapCommandName,
): DesktopDaemonBootstrapLaunchSpec {
  const { cwd, env, isPackaged, platform, repoRoot: launchRepoRoot, resourcesPath } = options;
  const launchSpec = resolveDesktopDaemonBootstrapLaunchSpec({
    commandName,
    cwd,
    env,
    isPackaged,
    platform,
    repoRoot: launchRepoRoot,
    resourcesPath,
  });

  assertDesktopBootstrapLaunchSpecExists(launchSpec, isPackaged);
  return launchSpec;
}

export async function startDaemonViaBootstrap(): Promise<DaemonControlStatusResult> {
  return await runDaemonBootstrapCommand(repoRoot, DAEMON_BOOTSTRAP_START_COMMAND);
}

export async function snapshotDaemonViaBootstrap(): Promise<DaemonControlStatusResult> {
  return await runDaemonBootstrapCommand(repoRoot, DAEMON_BOOTSTRAP_SNAPSHOT_COMMAND);
}

export async function reconcileDaemonViaBootstrap(): Promise<DaemonControlStatusResult> {
  return await runDaemonBootstrapCommand(repoRoot, DAEMON_BOOTSTRAP_RECONCILE_COMMAND);
}

export async function enableDaemonBackgroundModeViaBootstrap(): Promise<DaemonControlStatusResult> {
  return await runDaemonBootstrapCommand(repoRoot, DAEMON_BOOTSTRAP_ENABLE_BACKGROUND_COMMAND);
}

export async function disableDaemonBackgroundModeViaBootstrap(): Promise<DaemonControlStatusResult> {
  return await runDaemonBootstrapCommand(repoRoot, DAEMON_BOOTSTRAP_DISABLE_BACKGROUND_COMMAND);
}

export async function stopDaemonViaBootstrap(): Promise<DaemonControlStatusResult> {
  return await runDaemonBootstrapCommand(repoRoot, DAEMON_BOOTSTRAP_STOP_COMMAND);
}

async function runDaemonBootstrapCommand(
  launchRepoRoot: string,
  commandName: DaemonBootstrapCommandName,
): Promise<DaemonControlStatusResult> {
  const bootstrapConfig = createDesktopDaemonBootstrapConfig(process.env);
  const launchSpec = resolveDaemonBootstrapLaunchSpecForCommand(
    {
      cwd: process.cwd(),
      env: process.env,
      isPackaged: app.isPackaged,
      platform: process.platform,
      repoRoot: launchRepoRoot,
      resourcesPath: process.resourcesPath,
    },
    commandName,
  );

  const args = [...launchSpec.argsPrefix];
  const child = spawn(launchSpec.command, args, {
    cwd: launchSpec.cwd,
    env: bootstrapConfig.childEnv,
    stdio: ["ignore", "pipe", "pipe"],
  });

  return await new Promise<DaemonControlStatusResult>((resolve, reject) => {
    let stdout = "";
    let stderr = "";

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.once("error", (error) => {
      reject(new Error(`failed to start daemon bootstrap: ${error.message}`, { cause: error }));
    });
    child.once("exit", (code, signal) => {
      if (code !== 0) {
        reject(
          new Error(
            `daemon bootstrap exited with code ${code ?? "null"} signal ${signal ?? "none"}${stderr ? `: ${stderr.trim()}` : ""}`,
          ),
        );
        return;
      }
      try {
        resolve(parseDaemonControlStatusResult(JSON.parse(stdout.trim()) as unknown));
      } catch (error) {
        reject(
          error instanceof Error
            ? error
            : new Error("failed to parse daemon bootstrap control status"),
        );
      }
    });
  });
}
