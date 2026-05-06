import { describe, expect, it } from "vite-plus/test";

import {
  assertDesktopBootstrapLaunchSpecExists,
  buildDesktopDaemonChildEnv,
  createDesktopDaemonBootstrapConfig,
  resolveDesktopDaemonBootstrapLaunchSpec,
  resolveDesktopRepoRoot,
  resolvePackagedDaemonBinaryPath,
} from "../../packages/main/src/desktop-bootstrap-config.js";

describe("desktop-bootstrap-config", () => {
  it("keeps bootstrap env to an allowlisted daemon-oriented subset", () => {
    const childEnv = buildDesktopDaemonChildEnv({
      CARGO_TARGET_DIR: "/Volumes/SonnetSSD/taugentic-dev/tauri-agentic-targets/desktop-dev",
      HOME: "/Users/test",
      PATH: "/usr/bin:/bin",
      SECRET_OTHER_APP: "nope",
      TAUGENTIC_DAEMON_BINARY: "/tmp/ta-daemon",
      TAUGENTIC_DAEMON_SOCKET_NAME: "desktop.sock",
      TAUGENTIC_LOG_DIR: "/tmp/taugentic-logs",
    });

    expect(childEnv).toEqual({
      CARGO_TARGET_DIR: "/Volumes/SonnetSSD/taugentic-dev/tauri-agentic-targets/desktop-dev",
      HOME: "/Users/test",
      PATH: "/usr/bin:/bin",
      TAUGENTIC_DAEMON_BINARY: "/tmp/ta-daemon",
      TAUGENTIC_DAEMON_SOCKET_NAME: "desktop.sock",
      TAUGENTIC_LOG_DIR: "/tmp/taugentic-logs",
    });
  });

  it("keeps retained-state config roots for packaged bootstrap child env", () => {
    const childEnv = buildDesktopDaemonChildEnv({
      APPDATA: "C:/Users/test/AppData/Roaming",
      HOME: "/home/test",
      PATH: "/usr/bin:/bin",
      SECRET_OTHER_APP: "nope",
      USERPROFILE: "C:/Users/test",
      XDG_CONFIG_HOME: "/home/test/.config/taugentic",
      XDG_RUNTIME_DIR: "/run/user/1000",
    });

    expect(childEnv).toEqual({
      APPDATA: "C:/Users/test/AppData/Roaming",
      HOME: "/home/test",
      PATH: "/usr/bin:/bin",
      USERPROFILE: "C:/Users/test",
      XDG_CONFIG_HOME: "/home/test/.config/taugentic",
      XDG_RUNTIME_DIR: "/run/user/1000",
    });
  });

  it("normalizes empty daemon bootstrap override to undefined", () => {
    const config = createDesktopDaemonBootstrapConfig({
      TAUGENTIC_DAEMON_BINARY: "   ",
      TAUGENTIC_ALLOW_PACKAGED_DAEMON_BINARY_OVERRIDE: "1",
    });

    expect(config.daemonBinaryOverride).toBeUndefined();
    expect(config.allowPackagedOverride).toBe(true);
  });

  it("prefers explicit daemon binary override for bootstrap launch spec", () => {
    const spec = resolveDesktopDaemonBootstrapLaunchSpec({
      commandName: "start",
      cwd: "/repo",
      env: {
        TAUGENTIC_DAEMON_BINARY: "/tmp/ta-daemon",
      },
      isPackaged: false,
      platform: "darwin",
      repoRoot: "/repo",
      resourcesPath: "/resources",
    });

    expect(spec).toEqual({
      command: "/tmp/ta-daemon",
      argsPrefix: ["__runtime-control-bootstrap", "start"],
      cwd: "/tmp",
    });
  });

  it("resolves relative daemon binary overrides once against the launch cwd", () => {
    const spec = resolveDesktopDaemonBootstrapLaunchSpec({
      commandName: "start",
      cwd: "/repo",
      env: {
        TAUGENTIC_DAEMON_BINARY: "./target/debug/ta-daemon",
      },
      isPackaged: false,
      platform: "darwin",
      repoRoot: "/repo",
      resourcesPath: "/resources",
    });

    expect(spec).toEqual({
      command: "/repo/target/debug/ta-daemon",
      argsPrefix: ["__runtime-control-bootstrap", "start"],
      cwd: "/repo/target/debug",
    });
  });

  it("uses packaged daemon path when app is packaged", () => {
    const spec = resolveDesktopDaemonBootstrapLaunchSpec({
      commandName: "start",
      cwd: "/repo",
      env: {},
      isPackaged: true,
      platform: "darwin",
      repoRoot: "/repo",
      resourcesPath: "/Applications/Taugentic.app/Contents/Resources",
    });

    expect(spec).toEqual({
      command: "/Applications/Taugentic.app/Contents/Resources/bin/ta-daemon",
      argsPrefix: ["__runtime-control-bootstrap", "start"],
    });
  });

  it("ignores packaged binary override unless packaged override opt-in is set", () => {
    const spec = resolveDesktopDaemonBootstrapLaunchSpec({
      commandName: "start",
      cwd: "/repo",
      env: {
        TAUGENTIC_DAEMON_BINARY: "/tmp/override-daemon",
      },
      isPackaged: true,
      platform: "darwin",
      repoRoot: "/repo",
      resourcesPath: "/Applications/Taugentic.app/Contents/Resources",
    });

    expect(spec).toEqual({
      command: "/Applications/Taugentic.app/Contents/Resources/bin/ta-daemon",
      argsPrefix: ["__runtime-control-bootstrap", "start"],
    });
  });

  it("uses cargo bootstrap launch in dev mode", () => {
    const spec = resolveDesktopDaemonBootstrapLaunchSpec({
      commandName: "start",
      cwd: "/repo",
      env: {},
      isPackaged: false,
      platform: "darwin",
      repoRoot: "/repo",
      resourcesPath: "/resources",
    });

    expect(spec).toEqual({
      command: "cargo",
      argsPrefix: [
        "run",
        "--package",
        "ta-orchestrator",
        "--bin",
        "ta-daemon",
        "--",
        "__runtime-control-bootstrap",
        "start",
      ],
      cwd: "/repo",
    });
  });

  it("resolves packaged daemon binary path per platform", () => {
    expect(resolvePackagedDaemonBinaryPath("win32", "C:/Taugentic")).toBe(
      "C:\\Taugentic\\bin\\ta-daemon.exe",
    );
    expect(resolvePackagedDaemonBinaryPath("darwin", "/Applications/Taugentic")).toBe(
      "/Applications/Taugentic/bin/ta-daemon",
    );
  });

  it("resolves desktop repo root from the module url", () => {
    const repoRoot = resolveDesktopRepoRoot(
      "file:///Users/test/projects/taugentic/apps/desktop/packages/main/src/daemon-bootstrap.ts",
    );

    expect(repoRoot).toBe("/Users/test/projects/taugentic/");
  });

  it("rejects missing packaged daemon binary launch specs", () => {
    expect(() =>
      assertDesktopBootstrapLaunchSpecExists(
        {
          command: "/definitely/missing/ta-daemon",
          argsPrefix: ["__runtime-control-bootstrap", "start"],
        },
        true,
      ),
    ).toThrow("bundled ta-daemon executable is missing");
  });
});
