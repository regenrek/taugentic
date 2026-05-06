import { execFileSync, spawn } from "node:child_process";
import { createServer } from "node:net";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import process from "node:process";

import { resolveLauncherExitCode } from "./launcher-exit.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const desktopRoot = resolve(__dirname, "..");
const mainPackageDir = resolve(desktopRoot, "packages/main");
const repoRoot = resolve(desktopRoot, "..", "..");
const DEFAULT_ELECTRON_AGENT_LOG_FILE = "my-electron.log";

const mode = process.argv[2];
const rendererPort = process.env.TAUGENTIC_DESKTOP_PORT ?? "1420";
const rendererUrl = `http://127.0.0.1:${rendererPort}`;
const DAEMON_BOOTSTRAP_SUBCOMMAND = "__runtime-control-bootstrap";
const DAEMON_BOOTSTRAP_ACTIONS = {
  resetLocal: "reset-local",
  snapshot: "snapshot",
  stop: "stop",
};
const ORPHAN_PARENT_PROCESS_ID = 1;
const DESKTOP_DEV_ORPHAN_MARKERS = [
  "/vite-plus-core/dist/vite/node/cli.js dev",
  "/vite-plus-core/dist/vite/node/cli.js build --watch",
  "/vite-plus/bin/vp dev",
  "/vite-plus/bin/vp build",
  "/electron/dist/Electron.app/Contents/MacOS/Electron dist/index.js",
  "/scripts/launch-desktop.mjs dev",
];

const children = new Set();
let shuttingDown = false;
let shutdownExitCode = 0;

if (isLaunchDesktopEntrypoint()) {
  if (mode !== "start" && mode !== "dev") {
    console.error("usage: node ./scripts/launch-desktop.mjs <start|dev>");
    process.exit(1);
  }

  for (const signal of ["SIGINT", "SIGTERM"]) {
    process.on(signal, () => {
      void shutdown({ signal });
    });
  }

  process.on("exit", () => {
    if (!shuttingDown) {
      terminateChildren("SIGTERM");
    }
  });

  await run(mode);
}

export function isLaunchDesktopEntrypoint(argv = process.argv, importMetaUrl = import.meta.url) {
  const entryArg = argv[1];
  return typeof entryArg === "string" && resolve(entryArg) === fileURLToPath(importMetaUrl);
}

export function resolveDesktopDevPreflightCommands(rootDir = repoRoot) {
  return [
    {
      name: "daemon:stop",
      command: resolveDaemonBootstrapCommand(DAEMON_BOOTSTRAP_ACTIONS.stop),
      cwd: rootDir,
    },
    {
      name: "daemon:reset-local",
      command: resolveDaemonBootstrapCommand(DAEMON_BOOTSTRAP_ACTIONS.resetLocal),
      cwd: rootDir,
    },
    resolveDesktopDaemonCleanupStep(rootDir),
  ];
}

export function resolveDaemonBootstrapCommand(action) {
  return [
    "cargo",
    "run",
    "--package",
    "ta-orchestrator",
    "--bin",
    "ta-daemon",
    "--",
    DAEMON_BOOTSTRAP_SUBCOMMAND,
    action,
  ];
}

export function shouldForceCleanupForDaemonStatus(status) {
  return status.actualMode === "foreign";
}

export function parseDesktopDevOrphanProcessIds(processTable, scopeDir = desktopRoot) {
  return processTable
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .flatMap((line) => {
      const match = line.match(/^(\d+)\s+(\d+)\s+(.*)$/);
      if (!match) {
        return [];
      }

      const [, pidText, ppidText, command] = match;
      const pid = Number(pidText);
      const ppid = Number(ppidText);
      if (!Number.isInteger(pid) || !Number.isInteger(ppid)) {
        return [];
      }
      if (ppid !== ORPHAN_PARENT_PROCESS_ID) {
        return [];
      }
      if (!command.includes(scopeDir)) {
        return [];
      }
      if (!DESKTOP_DEV_ORPHAN_MARKERS.some((marker) => command.includes(marker))) {
        return [];
      }

      return [pid];
    });
}

export function resolveDesktopDaemonCleanupStep(
  rootDir = repoRoot,
  options = { includeProductSocket: false, name: "daemon:cleanup" },
) {
  return {
    name: options.name,
    command: [
      "node",
      resolve(rootDir, "scripts/daemon-cleanup.mjs"),
      "--apply",
      ...(options.includeProductSocket ? ["--include-product-socket"] : []),
    ],
    cwd: rootDir,
  };
}

export function resolveElectronLaunchArguments(env = process.env) {
  const args = ["./node_modules/.bin/electron", "dist/index.js"];
  const remoteDebuggingPort = readConfiguredPort(
    env.TAUGENTIC_ELECTRON_REMOTE_DEBUGGING_PORT,
    "TAUGENTIC_ELECTRON_REMOTE_DEBUGGING_PORT",
  );
  if (remoteDebuggingPort) {
    args.push(`--remote-debugging-port=${remoteDebuggingPort}`);
  }

  const inspectPort = readConfiguredPort(
    env.TAUGENTIC_ELECTRON_INSPECT_PORT,
    "TAUGENTIC_ELECTRON_INSPECT_PORT",
  );
  if (inspectPort) {
    args.push(`--inspect=${inspectPort}`);
  }

  if (isEnabledFlag(env.TAUGENTIC_ELECTRON_ENABLE_LOGGING)) {
    args.push("--enable-logging");
    args.push(`--log-file=${resolveElectronLogFile(env)}`);
  }

  return args;
}

async function runDesktopDevPreflight() {
  await cleanupDesktopDevOrphans();
  const preflightSteps = resolveDesktopDevPreflightCommands();
  try {
    await runDaemonBootstrapCommand(
      preflightSteps[0].name,
      DAEMON_BOOTSTRAP_ACTIONS.stop,
      preflightSteps[0].cwd,
    );
  } catch {
    await runCommand(
      "daemon:force-cleanup",
      resolveDesktopDaemonCleanupStep(repoRoot, {
        includeProductSocket: true,
        name: "daemon:force-cleanup",
      }).command,
      repoRoot,
    );
  }

  let resetStatus = await runDaemonBootstrapCommand(
    preflightSteps[1].name,
    DAEMON_BOOTSTRAP_ACTIONS.resetLocal,
    preflightSteps[1].cwd,
  );
  if (shouldForceCleanupForDaemonStatus(resetStatus)) {
    await runCommand(
      "daemon:force-cleanup",
      resolveDesktopDaemonCleanupStep(repoRoot, {
        includeProductSocket: true,
        name: "daemon:force-cleanup",
      }).command,
      repoRoot,
    );
    await runDaemonBootstrapCommand(
      "daemon:reset-local:retry",
      DAEMON_BOOTSTRAP_ACTIONS.resetLocal,
      preflightSteps[1].cwd,
    );
  }

  await runCommand(preflightSteps[2].name, preflightSteps[2].command, preflightSteps[2].cwd);

  const snapshot = await runDaemonBootstrapCommand(
    "daemon:snapshot",
    DAEMON_BOOTSTRAP_ACTIONS.snapshot,
    preflightSteps[1].cwd,
  );
  if (shouldForceCleanupForDaemonStatus(snapshot)) {
    throw new Error("desktop dev preflight left a foreign daemon runtime on the product socket");
  }
}

async function run(currentMode) {
  if (currentMode === "start") {
    await runDesktopBuild();
    spawnManaged("electron", electronArgs(), {
      cwd: mainPackageDir,
      onExit: (code, signal) =>
        void shutdown({ exitCode: code ?? undefined, signal: signal ?? undefined }),
    });
    return;
  }

  await runCommand(
    "shared",
    ["pnpm", "--filter", "@taugentic/desktop-shared", "build"],
    desktopRoot,
  );
  await runCommand(
    "preload",
    ["pnpm", "--filter", "@taugentic/desktop-preload", "build"],
    desktopRoot,
  );
  await runCommand("main", ["pnpm", "--filter", "@taugentic/desktop-main", "build"], desktopRoot);
  await runDesktopDevPreflight();

  spawnManaged(
    "shared:watch",
    ["pnpm", "exec", "tsc", "-p", "tsconfig.build.json", "--watch", "--preserveWatchOutput"],
    {
      cwd: resolve(desktopRoot, "packages/shared"),
    },
  );
  spawnManaged("preload:watch", ["pnpm", "exec", "vp", "build", "--watch"], {
    cwd: resolve(desktopRoot, "packages/preload"),
  });
  spawnManaged(
    "main:watch",
    ["pnpm", "exec", "tsc", "-p", "tsconfig.build.json", "--watch", "--preserveWatchOutput"],
    {
      cwd: resolve(desktopRoot, "packages/main"),
    },
  );
  await assertRendererPortAvailableForLaunch(rendererPort);
  spawnManaged(
    "renderer",
    ["pnpm", "exec", "vp", "dev", "--host", "127.0.0.1", "--port", rendererPort, "--strictPort"],
    {
      cwd: resolve(desktopRoot, "packages/renderer"),
    },
  );
  await waitForHttpReady(rendererUrl);

  spawnManaged("electron", electronArgs(), {
    cwd: mainPackageDir,
    env: { ...process.env, TAUGENTIC_DESKTOP_URL: rendererUrl },
    onExit: (code, signal) =>
      void shutdown({ exitCode: code ?? undefined, signal: signal ?? undefined }),
  });
}

async function cleanupDesktopDevOrphans(scopeDir = desktopRoot) {
  let processTable;
  try {
    processTable = execFileSync("ps", ["-axo", "pid=,ppid=,command="], {
      cwd: scopeDir,
      encoding: "utf8",
    });
  } catch (error) {
    console.warn(`[desktop:cleanup] failed to inspect process table: ${toErrorMessage(error)}`);
    return;
  }

  const orphanIds = parseDesktopDevOrphanProcessIds(processTable, scopeDir);
  if (orphanIds.length === 0) {
    return;
  }

  for (const pid of orphanIds) {
    try {
      process.kill(pid, "SIGTERM");
    } catch (_error) {
      void _error;
    }
  }

  await sleep(250);

  for (const pid of orphanIds) {
    try {
      process.kill(pid, 0);
    } catch {
      continue;
    }
    try {
      process.kill(pid, "SIGKILL");
    } catch (_error) {
      void _error;
    }
  }
}

async function assertRendererPortAvailableForLaunch(port) {
  await new Promise((resolvePromise, rejectPromise) => {
    const server = createServer();
    server.once("error", (error) => {
      rejectPromise(
        new Error(`desktop renderer port 127.0.0.1:${port} is already occupied`, {
          cause: error,
        }),
      );
    });
    server.listen({ host: "127.0.0.1", port: Number(port) }, () => {
      server.close((closeError) => {
        if (closeError) {
          rejectPromise(closeError);
          return;
        }
        resolvePromise(undefined);
      });
    });
  });
}

async function runDesktopBuild() {
  await runCommand("desktop:build", ["pnpm", "build"], desktopRoot);
}

function spawnManaged(name, command, options) {
  const child = spawn(command[0], command.slice(1), {
    cwd: options.cwd,
    env: options.env ?? process.env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  children.add(child);

  child.stdout.on("data", (chunk) => writePrefixed(name, chunk));
  child.stderr.on("data", (chunk) => writePrefixed(name, chunk));
  child.on("exit", (code, signal) => {
    children.delete(child);
    if (!shuttingDown && options.onExit) {
      options.onExit(code, signal);
    }
  });
  child.on("error", (error) => {
    console.error(`[${name}] ${error.message}`);
    void shutdown({ exitCode: 1 });
  });

  return child;
}

function electronArgs(env = process.env) {
  return resolveElectronLaunchArguments(env);
}

async function runCommand(name, command, cwd) {
  await new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command[0], command.slice(1), {
      cwd,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    child.stdout.on("data", (chunk) => writePrefixed(name, chunk));
    child.stderr.on("data", (chunk) => writePrefixed(name, chunk));
    child.on("exit", (code) => {
      if (code === 0) {
        resolvePromise();
        return;
      }
      rejectPromise(new Error(`${name} exited with code ${code ?? 1}`));
    });
    child.on("error", rejectPromise);
  });
}

async function runDaemonBootstrapCommand(name, action, cwd) {
  const command = resolveDaemonBootstrapCommand(action);
  const stdout = await runCommandCaptureStdout(name, command, cwd);
  const lines = stdout
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const payload = lines.at(-1);
  if (!payload) {
    throw new Error(`${name} produced no daemon bootstrap payload`);
  }
  return JSON.parse(payload);
}

async function runCommandCaptureStdout(name, command, cwd) {
  return await new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command[0], command.slice(1), {
      cwd,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString();
      writePrefixed(name, chunk);
    });
    child.stderr.on("data", (chunk) => writePrefixed(name, chunk));
    child.on("exit", (code) => {
      if (code === 0) {
        resolvePromise(stdout);
        return;
      }
      rejectPromise(new Error(`${name} exited with code ${code ?? 1}`));
    });
    child.on("error", rejectPromise);
  });
}

async function waitForHttpReady(url) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return;
      }
    } catch (_error) {
      void _error;
    }
    await sleep(500);
  }
  throw new Error(`renderer dev server did not become ready at ${url} within 30s`);
}

async function shutdown(exitStatus = {}) {
  if (shuttingDown) {
    return;
  }
  shuttingDown = true;
  shutdownExitCode = resolveLauncherExitCode(exitStatus);
  terminateChildren("SIGTERM");
  await sleep(250);
  process.exit(shutdownExitCode);
}

function terminateChildren(signal) {
  for (const child of children) {
    if (!child.killed) {
      child.kill(signal);
    }
  }
}

function toErrorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function writePrefixed(name, chunk) {
  const lines = chunk.toString().split(/\r?\n/).filter(Boolean);
  for (const line of lines) {
    console.log(`[${name}] ${line}`);
  }
}

function sleep(ms) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, ms));
}

function isEnabledFlag(value) {
  return value === "1" || value === "true";
}

function readConfiguredPort(value, name) {
  if (!value) {
    return undefined;
  }
  if (!/^\d+$/.test(value)) {
    throw new Error(`${name} must be a numeric port, received ${JSON.stringify(value)}`);
  }
  const port = Number(value);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(`${name} must be between 1 and 65535, received ${value}`);
  }
  return String(port);
}

function resolveElectronLogFile(env) {
  const configured = env.TAUGENTIC_ELECTRON_LOG_FILE;
  if (configured && configured.trim().length > 0) {
    return configured;
  }
  const tmpDir = env.TMPDIR && env.TMPDIR.trim().length > 0 ? env.TMPDIR : "/tmp";
  return resolve(tmpDir, DEFAULT_ELECTRON_AGENT_LOG_FILE);
}
