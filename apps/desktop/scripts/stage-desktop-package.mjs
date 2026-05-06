import { spawn } from "node:child_process";
import { chmod, cp, mkdir, mkdtemp, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  assertStageTargetPlatformConfiguration,
  buildStageAppPackageManifest,
  daemonBinaryFileNameForPlatform,
  desktopRootDir,
  repoRootDir,
  resolveDesktopReleaseProfile,
  resolveStageCargoTargetTriple,
  resolveStageTargetPlatform,
  stageCopyPlan,
  stagedAppDir,
  stagedResourcesDir,
} from "./package-layout.mjs";

export function resolveStageDesktopPackageOptions(
  argv = process.argv.slice(2),
  env = process.env,
  processPlatform = process.platform,
) {
  const args = new Set(argv);
  const targetPlatform = resolveStageTargetPlatform(argv, env, processPlatform);
  const cargoTargetTriple = resolveStageCargoTargetTriple(argv, env);
  assertStageTargetPlatformConfiguration(targetPlatform, processPlatform, cargoTargetTriple);
  return {
    cargoTargetTriple,
    releaseProfile: resolveDesktopReleaseProfile(argv, env),
    skipDaemonBuild: args.has("--skip-daemon-build"),
    skipDesktopBuild: args.has("--skip-desktop-build"),
    skipInstall: args.has("--skip-install"),
    targetPlatform,
  };
}

export async function stageDesktopPackage(options) {
  const stageRootDir = options.stageRootDir ?? path.dirname(stagedAppDir);
  const currentStagePaths = resolveStagePaths(stageRootDir);
  const pendingRootDir = await createPendingStageRoot(stageRootDir);
  const pendingStagePaths = resolveStagePaths(pendingRootDir);
  const runStageCommand = options.runCommand ?? runCommand;
  let keepPendingRoot = false;
  try {
    if (!options.skipDesktopBuild) {
      await runStageCommand("desktop:build", ["pnpm", "build"], desktopRootDir);
    }
    if (!options.skipDaemonBuild) {
      const cargoArgs = [
        "build",
        "--release",
        "--package",
        "ta-orchestrator",
        "--bin",
        "ta-daemon",
      ];
      if (options.cargoTargetTriple) {
        cargoArgs.push("--target", options.cargoTargetTriple);
      }
      await runStageCommand("daemon:build", ["cargo", ...cargoArgs], repoRootDir);
    }

    const stagePackageManifest = buildStageAppPackageManifest({
      releaseProfile: options.releaseProfile,
      version: await resolveStagePackageVersion(options.mainPackageVersion),
    });

    await mkdir(pendingStagePaths.stagedAppDir, { recursive: true });
    await mkdir(pendingStagePaths.stagedResourcesBinDir, { recursive: true });

    for (const entry of (options.copyPlanBuilder ?? stageCopyPlan)(
      pendingStagePaths.stagedAppDir,
    )) {
      await copyRequiredPath(entry.from, entry.to);
    }

    await writeFile(
      path.join(pendingStagePaths.stagedAppDir, "package.json"),
      `${JSON.stringify(stagePackageManifest, null, 2)}\n`,
      "utf8",
    );

    if (!options.skipInstall) {
      await runStageCommand(
        "stage:install",
        [
          "pnpm",
          "install",
          "--prod",
          "--ignore-workspace",
          "--no-frozen-lockfile",
          "--package-import-method",
          "copy",
        ],
        pendingStagePaths.stagedAppDir,
      );
    }

    const cargoTargetDir = await (options.resolveCargoTargetDir ?? resolveCargoTargetDir)();
    const daemonBinaryName = daemonBinaryFileNameForPlatform(options.targetPlatform);
    const builtDaemonBinary = options.cargoTargetTriple
      ? path.join(cargoTargetDir, options.cargoTargetTriple, "release", daemonBinaryName)
      : path.join(cargoTargetDir, "release", daemonBinaryName);
    const stagedDaemonBinary = path.join(pendingStagePaths.stagedResourcesBinDir, daemonBinaryName);
    await copyRequiredPath(builtDaemonBinary, stagedDaemonBinary);
    if (options.targetPlatform !== "win32") {
      await chmod(stagedDaemonBinary, 0o755);
    }

    await promoteStagePaths(
      stageRootDir,
      currentStagePaths,
      pendingStagePaths,
      options.renamePath ?? rename,
    );

    console.log(`staged app: ${currentStagePaths.stagedAppDir}`);
    console.log(
      `staged daemon binary: ${path.join(currentStagePaths.stagedResourcesBinDir, daemonBinaryName)}`,
    );
  } catch (error) {
    if (
      error instanceof Error &&
      error.message.startsWith("failed to restore previous staged package after promote error;")
    ) {
      keepPendingRoot = true;
    }
    throw error;
  } finally {
    if (!keepPendingRoot) {
      await rm(pendingRootDir, { recursive: true, force: true });
    }
  }
}

function resolveStagePaths(rootDir) {
  return {
    stagedAppDir: path.join(rootDir, path.basename(stagedAppDir)),
    stagedResourcesBinDir: path.join(rootDir, path.basename(stagedResourcesDir), "bin"),
    stagedResourcesDir: path.join(rootDir, path.basename(stagedResourcesDir)),
  };
}

async function createPendingStageRoot(stageRootDir) {
  await mkdir(stageRootDir, { recursive: true });
  return await mkdtemp(path.join(stageRootDir, ".pending-stage-"));
}

async function resolveStagePackageVersion(explicitVersion) {
  if (explicitVersion) {
    return explicitVersion;
  }
  const mainPackageManifest = JSON.parse(
    await readFile(path.join(desktopRootDir, "packages", "main", "package.json"), "utf8"),
  );
  return String(mainPackageManifest.version ?? "0.0.1");
}

async function promoteStagePaths(stageRootDir, currentStagePaths, pendingStagePaths, renamePath) {
  const backupRootDir = await mkdtemp(path.join(stageRootDir, ".backup-stage-"));
  const backupStagePaths = resolveStagePaths(backupRootDir);
  const promotedEntries = [];
  let keepBackupRoot = false;
  try {
    for (const entry of [
      {
        backupPath: backupStagePaths.stagedResourcesDir,
        currentPath: currentStagePaths.stagedResourcesDir,
        pendingPath: pendingStagePaths.stagedResourcesDir,
      },
      {
        backupPath: backupStagePaths.stagedAppDir,
        currentPath: currentStagePaths.stagedAppDir,
        pendingPath: pendingStagePaths.stagedAppDir,
      },
    ]) {
      const hadCurrent = await pathExists(entry.currentPath);
      if (hadCurrent) {
        await renamePath(entry.currentPath, entry.backupPath);
      }
      const promotedEntry = { ...entry, hadCurrent, pendingPromoted: false };
      promotedEntries.push(promotedEntry);
      await renamePath(entry.pendingPath, entry.currentPath);
      promotedEntry.pendingPromoted = true;
    }
  } catch (error) {
    const rollbackErrors = [];
    for (const entry of promotedEntries.reverse()) {
      try {
        if (entry.pendingPromoted) {
          await rm(entry.currentPath, { recursive: true, force: true });
        }
        if (entry.hadCurrent && (await pathExists(entry.backupPath))) {
          await renamePath(entry.backupPath, entry.currentPath);
        }
      } catch (rollbackError) {
        keepBackupRoot = true;
        rollbackErrors.push(rollbackError);
      }
    }
    if (rollbackErrors.length > 0) {
      const rollbackSummary = rollbackErrors
        .map((rollbackError) =>
          rollbackError instanceof Error ? rollbackError.message : String(rollbackError),
        )
        .join("; ");
      throw new Error(
        `failed to restore previous staged package after promote error; backup kept at ${backupRootDir}; restore errors: ${rollbackSummary}`,
        { cause: error },
      );
    }
    throw error;
  } finally {
    if (!keepBackupRoot) {
      await rm(backupRootDir, { recursive: true, force: true });
    }
  }
}

async function pathExists(targetPath) {
  return (await stat(targetPath).catch(() => null)) !== null;
}

async function copyRequiredPath(sourcePath, targetPath) {
  const sourceStat = await stat(sourcePath).catch(() => null);
  if (sourceStat === null) {
    throw new Error(`required staging input is missing: ${sourcePath}`);
  }
  await mkdir(path.dirname(targetPath), { recursive: true });
  if (sourceStat.isDirectory()) {
    await cp(sourcePath, targetPath, { force: true, recursive: true });
    return;
  }
  await cp(sourcePath, targetPath, { force: true });
}

async function runCommand(name, command, cwd) {
  await runCommandInternal(name, command, cwd, false);
}

async function runCommandCapture(name, command, cwd) {
  return runCommandInternal(name, command, cwd, true);
}

async function runCommandInternal(name, command, cwd, captureStdout) {
  return await new Promise((resolvePromise, rejectPromise) => {
    let stdout = "";
    const child = spawn(command[0], command.slice(1), {
      cwd,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    child.stdout.on("data", (chunk) => {
      if (captureStdout) {
        stdout += chunk.toString();
      } else {
        writePrefixed(name, chunk);
      }
    });
    child.stderr.on("data", (chunk) => writePrefixed(name, chunk));
    child.on("error", rejectPromise);
    child.on("exit", (code) => {
      if (code === 0) {
        resolvePromise(stdout);
        return;
      }
      rejectPromise(new Error(`${name} exited with code ${code ?? 1}`));
    });
  });
}

function writePrefixed(name, chunk) {
  const lines = chunk.toString().split(/\r?\n/).filter(Boolean);
  for (const line of lines) {
    console.log(`[${name}] ${line}`);
  }
}

async function resolveCargoTargetDir() {
  const explicitTargetDir = process.env.CARGO_TARGET_DIR?.trim();
  if (explicitTargetDir) {
    return explicitTargetDir;
  }
  const stdout = await runCommandCapture(
    "cargo:metadata",
    ["cargo", "metadata", "--format-version", "1", "--no-deps"],
    repoRootDir,
  );
  const metadata = JSON.parse(stdout);
  if (typeof metadata.target_directory !== "string" || !metadata.target_directory) {
    throw new Error("cargo metadata did not report a target_directory");
  }
  return metadata.target_directory;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  await stageDesktopPackage(resolveStageDesktopPackageOptions());
}
