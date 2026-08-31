import { spawn } from "node:child_process";
import { stat } from "node:fs/promises";

export const developmentTerminalDiagnostic =
  "Taugentic Development requires stdout and stderr to resolve to /dev/tty* character devices.";

function failTerminalResolution() {
  throw new Error(developmentTerminalDiagnostic);
}

function ttyForDescriptor(descriptor) {
  return new Promise((resolve, reject) => {
    const tty = spawn("/usr/bin/tty", [], {
      stdio: [descriptor, "pipe", "pipe"],
    });
    let output = "";

    tty.stdout.setEncoding("utf8");
    tty.stdout.on("data", (chunk) => {
      output += chunk;
    });
    tty.stderr.resume();
    tty.once("error", reject);
    tty.once("close", (code) => {
      if (code === 0) resolve(output);
      else reject(new Error("tty failed"));
    });
  });
}

async function resolveTerminalPath(descriptor, { runTtyImpl, statImpl }) {
  let terminalPath;
  try {
    terminalPath = (await runTtyImpl(descriptor)).trim();
  } catch {
    failTerminalResolution();
  }

  if (!terminalPath.startsWith("/dev/tty")) failTerminalResolution();

  let terminalStats;
  try {
    terminalStats = await statImpl(terminalPath);
  } catch {
    failTerminalResolution();
  }

  if (!terminalStats.isCharacterDevice()) failTerminalResolution();
  return terminalPath;
}

export async function resolveDevelopmentTerminalPaths({
  runTtyImpl = ttyForDescriptor,
  statImpl = stat,
  stdoutDescriptor = process.stdout.fd,
  stderrDescriptor = process.stderr.fd,
} = {}) {
  const [stdoutPath, stderrPath] = await Promise.all([
    resolveTerminalPath(stdoutDescriptor, { runTtyImpl, statImpl }),
    resolveTerminalPath(stderrDescriptor, { runTtyImpl, statImpl }),
  ]);

  return { stdoutPath, stderrPath };
}
