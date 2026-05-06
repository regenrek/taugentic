#!/usr/bin/env node

import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import process from "node:process";
import { spawnSync } from "node:child_process";

const apply = process.argv.includes("--apply");
const includeProductSocket = process.argv.includes("--include-product-socket");
const socketName = (process.env.TAUGENTIC_DAEMON_SOCKET_NAME ?? "ta-daemon").trim() || "ta-daemon";
const productSocketPath = resolveProductSocketPath(socketName);
const launchAgentLabel = "com.taugentic.daemon";
const systemdUserUnitName = "taugentic-daemon.service";

if (isDaemonCleanupEntrypoint()) {
  if (process.platform === "win32") {
    console.error("daemon-cleanup is only supported on unix hosts");
    process.exit(1);
  }

  const processes = listDaemonProcesses();
  const candidates = processes.filter(
    (entry) => includeProductSocket || !entry.socketPaths.includes(productSocketPath),
  );

  console.log(`product socket: ${productSocketPath}`);
  console.log(`include product socket: ${includeProductSocket ? "yes" : "no"}`);
  if (candidates.length === 0) {
    console.log("no stale ta-daemon candidates found");
    process.exit(0);
  }

  console.log(`found ${candidates.length} stale ta-daemon candidate(s):`);
  for (const entry of candidates) {
    console.log(`- pid ${entry.pid}`);
    console.log(`  command: ${entry.command}`);
    if (entry.socketPaths.length > 0) {
      console.log(`  sockets: ${entry.socketPaths.join(", ")}`);
    }
  }

  if (!apply) {
    console.log("\ndry run only. Re-run with --apply to terminate candidates and remove stale sockets.");
    process.exit(0);
  }

  if (includeProductSocket) {
    disableManagedBackgroundService();
  }

  for (const entry of candidates) {
    terminateProcess(entry.pid, "TERM");
  }

  sleep(1000);

  for (const entry of candidates) {
    if (isProcessAlive(entry.pid)) {
      terminateProcess(entry.pid, "KILL");
    }
  }

  const staleSocketPaths = new Set(
    candidates.flatMap((entry) =>
      entry.socketPaths.filter(
        (socketPath) =>
          socketPath && (includeProductSocket || socketPath !== productSocketPath),
      ),
    ),
  );

  for (const socketPath of staleSocketPaths) {
    if (!fs.existsSync(socketPath)) {
      continue;
    }
    const status = await probeUnixSocket(socketPath);
    if (status === "stale") {
      fs.rmSync(socketPath, { force: true });
      console.log(`removed stale socket ${socketPath}`);
    }
  }

  console.log("daemon cleanup applied");
}

export function isDaemonCleanupEntrypoint(argv = process.argv, importMetaUrl = import.meta.url) {
  const entryArg = argv[1];
  return typeof entryArg === "string" && path.resolve(entryArg) === fileURLToPath(importMetaUrl);
}

function disableManagedBackgroundService() {
  if (process.platform === "darwin") {
    const plistPath = path.join(os.homedir(), "Library", "LaunchAgents", `${launchAgentLabel}.plist`);
    const domain = resolveLaunchAgentDomain();
    const bootout = spawnSync("launchctl", ["bootout", domain, plistPath], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    if ((bootout.status ?? 1) === 0) {
      console.log(`booted out launch agent ${launchAgentLabel}`);
    }
    if (fs.existsSync(plistPath)) {
      fs.rmSync(plistPath, { force: true });
      console.log(`removed launch agent plist ${plistPath}`);
    }
    return;
  }

  if (process.platform === "linux") {
    const unitPath = path.join(os.homedir(), ".config", "systemd", "user", systemdUserUnitName);
    for (const args of [
      ["--user", "stop", systemdUserUnitName],
      ["--user", "disable", systemdUserUnitName],
    ]) {
      spawnSync("systemctl", args, {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      });
    }
    if (fs.existsSync(unitPath)) {
      fs.rmSync(unitPath, { force: true });
      console.log(`removed systemd user unit ${unitPath}`);
    }
    spawnSync("systemctl", ["--user", "daemon-reload"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
  }
}

function resolveProductSocketPath(name) {
  const xdgRuntimeDir = process.env.XDG_RUNTIME_DIR?.trim();
  if (process.platform === "darwin") {
    const runtimeDir =
      xdgRuntimeDir || path.join(os.homedir(), "Library", "Application Support", "taugentic", "runtime");
    return path.join(runtimeDir, `${name}.sock`);
  }
  if (process.platform === "linux") {
    const runtimeDir = xdgRuntimeDir || path.join(os.tmpdir(), `taugentic-uid-${process.getuid()}`);
    return path.join(runtimeDir, `${name}.sock`);
  }
  return path.join(os.tmpdir(), `${name}.sock`);
}

function resolveLaunchAgentDomain() {
  const uid = process.env.UID?.trim();
  if (uid) {
    return `gui/${uid}`;
  }
  const id = spawnSync("id", ["-u"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
  const detectedUid = id.stdout.trim();
  return detectedUid ? `gui/${detectedUid}` : "gui/501";
}

export function parseDaemonProcessEntries(processTable) {
  return processTable
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const firstSpace = line.indexOf(" ");
      if (firstSpace === -1) {
        return null;
      }
      const pid = Number(line.slice(0, firstSpace));
      const command = line.slice(firstSpace + 1).trim();
      return { pid, command };
    })
    .filter((entry) => entry !== null)
    .filter((entry) => Number.isInteger(entry.pid) && isDaemonCommand(entry.command));
}

export function isDaemonCommand(command) {
  const executable = command.trim().split(/\s+/u)[0] ?? "";
  return path.basename(executable) === "ta-daemon";
}

function listDaemonProcesses() {
  const ps = spawnSync("ps", ["-axo", "pid=,command="], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
  if ((ps.status ?? 1) !== 0) {
    throw new Error("failed to enumerate processes with ps");
  }
  return parseDaemonProcessEntries(ps.stdout)
    .map((entry) => ({
      ...entry,
      socketPaths: listSocketPathsForPid(entry.pid),
    }));
}

function listSocketPathsForPid(pid) {
  const lsof = spawnSync("lsof", ["-a", "-p", String(pid), "-U", "-Fn"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
  if ((lsof.status ?? 1) !== 0) {
    return [];
  }
  return lsof.stdout
    .split(/\r?\n/)
    .filter((line) => line.startsWith("n"))
    .map((line) => line.slice(1))
    .filter((value) => value.startsWith("/"));
}

function terminateProcess(pid, signal) {
  const kill = spawnSync("kill", [`-${signal}`, String(pid)], {
    stdio: ["ignore", "inherit", "inherit"],
  });
  if ((kill.status ?? 1) === 0) {
    console.log(`sent SIG${signal} to pid ${pid}`);
  }
}

function isProcessAlive(pid) {
  const result = spawnSync("kill", ["-0", String(pid)], {
    stdio: ["ignore", "ignore", "ignore"],
  });
  return (result.status ?? 1) === 0;
}

function sleep(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

function probeUnixSocket(socketPath) {
  return new Promise((resolve) => {
    const socket = net.createConnection(socketPath);
    let settled = false;
    const finish = (status) => {
      if (settled) {
        return;
      }
      settled = true;
      socket.destroy();
      resolve(status);
    };
    socket.once("connect", () => finish("live"));
    socket.once("error", (error) => {
      if (error.code === "ECONNREFUSED" || error.code === "ENOENT") {
        finish("stale");
        return;
      }
      finish("unknown");
    });
    socket.setTimeout(500, () => finish("unknown"));
  });
}
