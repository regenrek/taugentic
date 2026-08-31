import { execFile } from "node:child_process";
import { constants } from "node:fs";
import {
  access,
  mkdtemp,
  mkdir,
  readFile,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";
import { describe, expect, it } from "bun:test";

import {
  developmentAppName,
  developmentBundleIdentifier,
  macosDevelopmentAppLaunch,
  materializeMacosDevelopmentApp,
} from "../scripts/macos-development-app.mjs";

const execFileAsync = promisify(execFile);

describe("macOS development app materializer", () => {
  it("creates one ignored app whose matching executable is a copied Bun runtime", async () => {
    const fixtureRoot = await mkdtemp(
      join(tmpdir(), "taugentic-development-app-"),
    );
    const desktopRoot = join(fixtureRoot, "desktop");
    const sourceBun = join(fixtureRoot, "bun");

    try {
      await writeFile(sourceBun, "copied Bun runtime", { mode: 0o755 });
      const app = await materializeMacosDevelopmentApp({
        desktopRoot,
        executableSourcePath: sourceBun,
        entrypointPath: join(desktopRoot, "src/main.tsx"),
      });
      const executable = await readFile(app.executablePath, "utf8");
      const plist = await readFile(
        join(app.bundlePath, "Contents", "Info.plist"),
        "utf8",
      );
      const executableMode = (await stat(app.executablePath)).mode;

      expect(app.bundlePath).toBe(
        join(
          desktopRoot,
          ".taugentic-development",
          `${developmentAppName}.app`,
        ),
      );
      expect(app.entrypointPath).toBe(join(desktopRoot, "src/main.tsx"));
      expect(executable).toBe("copied Bun runtime");
      expect(executableMode & 0o111).not.toBe(0);
      expect(plist).toContain(
        `<key>CFBundleExecutable</key>\n  <string>${developmentAppName}</string>`,
      );
      expect(plist).toContain(
        `<key>CFBundleIdentifier</key>\n  <string>${developmentBundleIdentifier}</string>`,
      );
      expect(plist).toContain(
        "<key>NSMicrophoneUsageDescription</key>\n  <string>Taugentic uses the microphone only while you record a voice session.</string>",
      );
      expect(plist).toContain(
        "<key>NSScreenCaptureUsageDescription</key>\n  <string>Taugentic uses screen capture only when you explicitly request it.</string>",
      );
      await access(app.executablePath, constants.X_OK);
    } finally {
      await rm(fixtureRoot, { recursive: true, force: true });
    }
  });

  it("constructs the one LaunchServices command with daemon environment and Bun arguments", async () => {
    const developmentApp = {
      bundlePath: "/tmp/Taugentic Development.app",
      executablePath:
        "/tmp/Taugentic Development.app/Contents/MacOS/Taugentic Development",
      entrypointPath: "/workspace/desktop/src/main.tsx",
    };

    expect(
      macosDevelopmentAppLaunch({
        developmentApp,
        desktopRoot: "/workspace/desktop",
        daemonBinary: "/workspace/ta-daemon",
        daemonSocketName: "taugentic-fresh-test-socket",
        hot: true,
      }),
    ).toEqual({
      command: "/usr/bin/open",
      arguments: [
        "-n",
        "-W",
        "--env",
        "TAUGENTIC_DAEMON_BINARY=/workspace/ta-daemon",
        "--env",
        "TAUGENTIC_DAEMON_SOCKET_NAME=taugentic-fresh-test-socket",
        "/tmp/Taugentic Development.app",
        "--args",
        "--cwd",
        "/workspace/desktop",
        "--hot",
        "/workspace/desktop/src/main.tsx",
      ],
    });

    expect(
      macosDevelopmentAppLaunch({
        developmentApp,
        desktopRoot: "/workspace/desktop",
        daemonBinary: "/workspace/ta-daemon",
        daemonSocketName: undefined,
        hot: false,
      }).arguments,
    ).not.toContain("--hot");

    const launchWithoutSocket = macosDevelopmentAppLaunch({
      developmentApp,
      desktopRoot: "/workspace/desktop",
      daemonBinary: "/workspace/ta-daemon",
      daemonSocketName: undefined,
      hot: false,
    });
    expect(
      launchWithoutSocket.arguments.some((argument) =>
        argument.startsWith("TAUGENTIC_DAEMON_SOCKET_NAME="),
      ),
    ).toBe(false);

    expect(
      macosDevelopmentAppLaunch({
        developmentApp,
        desktopRoot: "/workspace/desktop",
        daemonBinary: "/workspace/ta-daemon",
        daemonSocketName: "taugentic-fresh-test-socket",
        hot: true,
        forwardStandardStreams: true,
        standardErrorPath: "/dev/ttys002",
        standardOutputPath: "/dev/ttys001",
      }).arguments,
    ).toEqual([
      "-n",
        "-W",
        "--stdout",
        "/dev/ttys001",
        "--stderr",
        "/dev/ttys002",
      "--env",
      "TAUGENTIC_DAEMON_BINARY=/workspace/ta-daemon",
      "--env",
      "TAUGENTIC_DAEMON_SOCKET_NAME=taugentic-fresh-test-socket",
      "/tmp/Taugentic Development.app",
      "--args",
      "--cwd",
      "/workspace/desktop",
      "--hot",
      "/workspace/desktop/src/main.tsx",
    ]);

    const launcherSource = await readFile(
      new URL("../scripts/start-desktop.mjs", import.meta.url),
      "utf8",
    );
    expect(launcherSource).toContain("spawn(launch.command, launch.arguments");
    expect(launcherSource).toContain("forwardStandardStreams: true");
    expect(launcherSource).toContain("resolveDevelopmentTerminalPaths()");
    expect(launcherSource).not.toContain("spawn(developmentApp.executablePath");
  });

  it("keeps debug and release build ownership in the desktop launcher", async () => {
    const fixtureRoot = await mkdtemp(join(tmpdir(), "taugentic-launcher-"));
    const binRoot = join(fixtureRoot, "bin");
    const targetRoot = join(fixtureRoot, "target");
    const logPath = join(fixtureRoot, "commands.jsonl");
    const repositoryRoot = resolve(import.meta.dirname, "../../..");
    const launcherPath = resolve(
      import.meta.dirname,
      "../scripts/start-desktop.mjs",
    );
    const originalPath = process.env.PATH ?? "";

    try {
      await mkdir(binRoot, { recursive: true });
      await mkdir(join(targetRoot, "debug"), { recursive: true });
      await mkdir(join(targetRoot, "release"), { recursive: true });
      await writeFile(join(targetRoot, "debug", "ta-daemon"), "debug", {
        mode: 0o755,
      });
      await writeFile(join(targetRoot, "release", "ta-daemon"), "release", {
        mode: 0o755,
      });
      await writeFile(
        join(binRoot, "cargo"),
        `#!/usr/bin/env node
import { appendFileSync } from "node:fs";
const args = process.argv.slice(2);
if (args[0] === "metadata") {
  process.stdout.write(${JSON.stringify(JSON.stringify({ target_directory: targetRoot }))});
} else {
  appendFileSync(process.env.TAUGENTIC_LAUNCH_TEST_LOG, JSON.stringify({ tool: "cargo", args }) + "\\n");
}
`,
        { mode: 0o755 },
      );
      await writeFile(
        join(binRoot, "pnpm"),
        `#!/usr/bin/env node
import { appendFileSync } from "node:fs";
appendFileSync(process.env.TAUGENTIC_LAUNCH_TEST_LOG, JSON.stringify({ tool: "pnpm", args: process.argv.slice(2), profile: process.env.CARGO_PROFILE ?? null }) + "\\n");
`,
        { mode: 0o755 },
      );

      const environment = {
        ...process.env,
        PATH: `${binRoot}:${originalPath}`,
        TAUGENTIC_LAUNCH_TEST_LOG: logPath,
      };
      const readCommands = async () =>
        (await readFile(logPath, "utf8"))
          .trim()
          .split("\n")
          .filter(Boolean)
          .map((line) => JSON.parse(line));

      await execFileAsync("just", ["desktop-dev"], {
        cwd: repositoryRoot,
        env: environment,
      });
      expect(await readCommands()).toEqual([
        { tool: "pnpm", args: ["--dir", "apps/desktop", "dev"], profile: null },
      ]);

      await writeFile(logPath, "");
      const launcherSource = await readFile(launcherPath, "utf8");
      expect(launcherSource).toContain('const profile = release ? "release" : "debug"');
      expect(launcherSource).toContain("const terminalPaths = await resolveDevelopmentTerminalPaths()");
      expect(launcherSource).toContain("await prepareDesktopArtifacts()");
      expect(launcherSource.indexOf("resolveDevelopmentTerminalPaths()")).toBeLessThan(
        launcherSource.indexOf("await prepareDesktopArtifacts()"),
      );

      await writeFile(logPath, "");
      await execFileAsync("just", ["desktop-release"], {
        cwd: repositoryRoot,
        env: environment,
      });
      expect(await readCommands()).toEqual([
        { tool: "pnpm", args: ["--dir", "apps/desktop", "start"], profile: null },
      ]);

      expect(launcherSource).toContain('profile === "release" ? ["--release"] : []');
    } finally {
      await rm(fixtureRoot, { recursive: true, force: true });
    }
  }, 20_000);
});
