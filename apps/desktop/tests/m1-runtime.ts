import { launch, type App as AutomationApp } from "@gpuix/react/automation"
import { execFileSync } from "node:child_process"
import { mkdtemp, rm } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

function daemonBinary(): string {
  const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--format-version=1", "--no-deps"], { encoding: "utf8" })) as { target_directory: string }
  const profile = process.env.TAUGENTIC_M1_DAEMON_PROFILE === "release" ? "release" : "debug"
  return resolve(metadata.target_directory, profile, process.platform === "win32" ? "ta-daemon.exe" : "ta-daemon")
}

export async function launchM1Desktop(): Promise<{ app: AutomationApp; close(): Promise<void> }> {
  const desktopRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..")
  const isolatedHome = await mkdtemp("/tmp/tg-m1-")
  const isolatedRuntime = await mkdtemp("/tmp/tg-m1-")
  const isolatedLogs = await mkdtemp("/tmp/tg-m1-")
  const socketName = `tg-m1-${process.pid}`
  const app = await launch({
    command: "bun",
    args: ["src/main.tsx"],
    cwd: desktopRoot,
    env: {
      HOME: isolatedHome,
      USERPROFILE: isolatedHome,
      XDG_CONFIG_HOME: join(isolatedHome, ".config"),
      XDG_RUNTIME_DIR: isolatedRuntime,
      APPDATA: join(isolatedHome, "AppData", "Roaming"),
      TAUGENTIC_DAEMON_SOCKET_NAME: socketName,
      TAUGENTIC_DAEMON_RUNTIME_MODE: "local",
      TAUGENTIC_LOG_DIR: isolatedLogs,
      TAUGENTIC_LOG_STDERR: "0",
      TAUGENTIC_WORKSPACE_PATH: desktopRoot,
      TAUGENTIC_DAEMON_BINARY: daemonBinary(),
    },
  })

  return {
    app,
    async close() {
      try {
        await app.close()
      } finally {
        await rm(isolatedHome, { recursive: true, force: true })
        await rm(isolatedRuntime, { recursive: true, force: true })
        await rm(isolatedLogs, { recursive: true, force: true })
      }
    },
  }
}
