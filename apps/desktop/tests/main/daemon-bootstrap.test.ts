import { EventEmitter } from "node:events";

import type { DaemonControlStatusResult } from "../../packages/shared/src/contracts.js";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

function createControlState(
  overrides: Partial<DaemonControlStatusResult> = {},
): DaemonControlStatusResult {
  return {
    actualMode: "local",
    allowedActions: ["stop", "enableBackground"],
    backgroundOptIn: false,
    daemonVersion: "0.0.1-test",
    desiredMode: "local",
    errorCode: null,
    logPath: "/tmp/taugentic-test.log",
    message: "Local mode is the desired runtime.",
    pendingTransition: null,
    protocolVersion: "2026-04-stage2",
    reconcileRequired: false,
    socketPath: "/tmp/taugentic-test.sock",
    transitionStatus: "idle",
    ...overrides,
  };
}

function createFakeReadable() {
  const stream = new EventEmitter() as EventEmitter & {
    setEncoding: ReturnType<typeof vi.fn>;
  };
  stream.setEncoding = vi.fn();
  return stream;
}

function createFakeChildProcess() {
  const child = new EventEmitter() as EventEmitter & {
    stderr: ReturnType<typeof createFakeReadable>;
    stdout: ReturnType<typeof createFakeReadable>;
  };
  child.stdout = createFakeReadable();
  child.stderr = createFakeReadable();
  return child;
}

const hoisted = vi.hoisted(() => ({
  app: {
    isPackaged: true,
  },
  existsSync: vi.fn(() => true),
  spawn: vi.fn(),
}));

vi.mock("electron", () => ({
  app: hoisted.app,
}));

vi.mock("node:fs", async (importOriginal) => {
  const actual = await importOriginal<typeof import("node:fs")>();
  return {
    ...actual,
    existsSync: hoisted.existsSync,
  };
});

vi.mock("node:child_process", async (importOriginal) => {
  const actual = await importOriginal<typeof import("node:child_process")>();
  return {
    ...actual,
    spawn: hoisted.spawn,
  };
});

describe("daemon-bootstrap", () => {
  const resourcesPathDescriptor = Object.getOwnPropertyDescriptor(process, "resourcesPath");
  const processWithResourcesPath = process as NodeJS.Process & {
    resourcesPath?: string | undefined;
  };
  const originalBootstrapEnv = {
    APPDATA: process.env.APPDATA,
    CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR,
    HOME: process.env.HOME,
    PATH: process.env.PATH,
    TAUGENTIC_DAEMON_BINARY: process.env.TAUGENTIC_DAEMON_BINARY,
    TAUGENTIC_DAEMON_SOCKET_NAME: process.env.TAUGENTIC_DAEMON_SOCKET_NAME,
    TAUGENTIC_LOG_DIR: process.env.TAUGENTIC_LOG_DIR,
    TEMP: process.env.TEMP,
    TMP: process.env.TMP,
    TMPDIR: process.env.TMPDIR,
    USERPROFILE: process.env.USERPROFILE,
    XDG_CONFIG_HOME: process.env.XDG_CONFIG_HOME,
    XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR,
  };

  beforeEach(() => {
    vi.resetModules();
    hoisted.app.isPackaged = true;
    hoisted.existsSync.mockReset();
    hoisted.existsSync.mockReturnValue(true);
    hoisted.spawn.mockReset();
    delete process.env.TAUGENTIC_DAEMON_BINARY;
    delete process.env.TAUGENTIC_ALLOW_PACKAGED_DAEMON_BINARY_OVERRIDE;
    delete process.env.TAUGENTIC_DAEMON_SOCKET_NAME;
    delete process.env.TAUGENTIC_LOG_DIR;
    delete process.env.APPDATA;
    delete process.env.CARGO_TARGET_DIR;
    delete process.env.TEMP;
    delete process.env.TMP;
    delete process.env.TMPDIR;
    delete process.env.USERPROFILE;
    delete process.env.XDG_CONFIG_HOME;
    delete process.env.XDG_RUNTIME_DIR;
    Object.defineProperty(process, "resourcesPath", {
      configurable: true,
      value: "/Applications/Taugentic.app/Contents/Resources",
    });
  });

  afterEach(() => {
    for (const [key, value] of Object.entries(originalBootstrapEnv)) {
      if (value == null) {
        delete process.env[key];
        continue;
      }
      process.env[key] = value;
    }
    if (resourcesPathDescriptor) {
      Object.defineProperty(process, "resourcesPath", resourcesPathDescriptor);
      return;
    }
    Reflect.deleteProperty(processWithResourcesPath, "resourcesPath");
  });

  it("uses the bundled daemon binary for packaged bootstrap commands", async () => {
    const child = createFakeChildProcess();
    const controlState = createControlState();
    hoisted.spawn.mockReturnValue(child);

    const { startDaemonViaBootstrap } = await import("../../packages/main/src/daemon-bootstrap.js");

    const resultPromise = startDaemonViaBootstrap();

    expect(hoisted.spawn).toHaveBeenCalledWith(
      "/Applications/Taugentic.app/Contents/Resources/bin/ta-daemon",
      ["__runtime-control-bootstrap", "start"],
      expect.objectContaining({
        env: expect.any(Object),
        stdio: ["ignore", "pipe", "pipe"],
      }),
    );
    expect(child.stdout.setEncoding).toHaveBeenCalledWith("utf8");
    expect(child.stderr.setEncoding).toHaveBeenCalledWith("utf8");

    child.stdout.emit("data", `${JSON.stringify(controlState)}\n`);
    child.emit("exit", 0, null);

    await expect(resultPromise).resolves.toEqual(controlState);
  });

  it("forwards only the allowlisted retained-state roots and daemon prefixes to the packaged bootstrap child", async () => {
    const child = createFakeChildProcess();
    const controlState = createControlState();
    hoisted.spawn.mockReturnValue(child);
    process.env.APPDATA = "C:/Users/test/AppData/Roaming";
    process.env.CARGO_TARGET_DIR =
      "/Volumes/SonnetSSD/taugentic-dev/tauri-agentic-targets/desktop-dev";
    process.env.HOME = "/home/test";
    process.env.PATH = "/usr/bin:/bin";
    process.env.SECRET_OTHER_APP = "do-not-forward";
    process.env.TAUGENTIC_DAEMON_SOCKET_NAME = "desktop.sock";
    process.env.TAUGENTIC_LOG_DIR = "/tmp/taugentic-logs";
    process.env.TMPDIR = "/tmp/bootstrap";
    process.env.USERPROFILE = "C:/Users/test";
    process.env.XDG_CONFIG_HOME = "/home/test/.config/taugentic";
    process.env.XDG_RUNTIME_DIR = "/run/user/1000";

    const { startDaemonViaBootstrap } = await import("../../packages/main/src/daemon-bootstrap.js");

    const resultPromise = startDaemonViaBootstrap();

    expect(hoisted.spawn).toHaveBeenCalledWith(
      "/Applications/Taugentic.app/Contents/Resources/bin/ta-daemon",
      ["__runtime-control-bootstrap", "start"],
      {
        cwd: undefined,
        env: {
          APPDATA: "C:/Users/test/AppData/Roaming",
          CARGO_TARGET_DIR: "/Volumes/SonnetSSD/taugentic-dev/tauri-agentic-targets/desktop-dev",
          HOME: "/home/test",
          PATH: "/usr/bin:/bin",
          TAUGENTIC_DAEMON_SOCKET_NAME: "desktop.sock",
          TAUGENTIC_LOG_DIR: "/tmp/taugentic-logs",
          TMPDIR: "/tmp/bootstrap",
          USERPROFILE: "C:/Users/test",
          XDG_CONFIG_HOME: "/home/test/.config/taugentic",
          XDG_RUNTIME_DIR: "/run/user/1000",
        },
        stdio: ["ignore", "pipe", "pipe"],
      },
    );

    child.stdout.emit("data", `${JSON.stringify(controlState)}\n`);
    child.emit("exit", 0, null);

    await expect(resultPromise).resolves.toEqual(controlState);
    expect(process.env.SECRET_OTHER_APP).toBe("do-not-forward");
    delete process.env.SECRET_OTHER_APP;
  });

  it("fails before spawn when the bundled daemon binary is missing in packaged mode", async () => {
    hoisted.existsSync.mockReturnValue(false);

    const { startDaemonViaBootstrap } = await import("../../packages/main/src/daemon-bootstrap.js");

    await expect(startDaemonViaBootstrap()).rejects.toThrow(
      "bundled ta-daemon executable is missing",
    );
    expect(hoisted.spawn).not.toHaveBeenCalled();
  });

  it("fails when the packaged daemon bootstrap child emits an error", async () => {
    const child = createFakeChildProcess();
    hoisted.spawn.mockReturnValue(child);

    const { startDaemonViaBootstrap } = await import("../../packages/main/src/daemon-bootstrap.js");

    const resultPromise = startDaemonViaBootstrap();
    child.emit("error", new Error("spawn failed"));

    await expect(resultPromise).rejects.toThrow("failed to start daemon bootstrap: spawn failed");
  });

  it("fails when the packaged daemon bootstrap exits non-zero", async () => {
    const child = createFakeChildProcess();
    hoisted.spawn.mockReturnValue(child);

    const { startDaemonViaBootstrap } = await import("../../packages/main/src/daemon-bootstrap.js");

    const resultPromise = startDaemonViaBootstrap();
    child.stderr.emit("data", "permission denied\n");
    child.emit("exit", 23, null);

    await expect(resultPromise).rejects.toThrow(
      "daemon bootstrap exited with code 23 signal none: permission denied",
    );
  });

  it("fails when the packaged daemon bootstrap exits with invalid control-status JSON", async () => {
    const child = createFakeChildProcess();
    hoisted.spawn.mockReturnValue(child);

    const { startDaemonViaBootstrap } = await import("../../packages/main/src/daemon-bootstrap.js");

    const resultPromise = startDaemonViaBootstrap();
    child.stdout.emit("data", "{not-json}\n");
    child.emit("exit", 0, null);

    await expect(resultPromise).rejects.toBeInstanceOf(SyntaxError);
  });
});
