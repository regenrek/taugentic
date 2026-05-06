import { resolve } from "node:path";

import { describe, expect, it } from "vite-plus/test";

import {
  isLaunchDesktopEntrypoint,
  parseDesktopDevOrphanProcessIds,
  resolveDaemonBootstrapCommand,
  resolveDesktopDaemonCleanupStep,
  resolveDesktopDevPreflightCommands,
  resolveElectronLaunchArguments,
  shouldForceCleanupForDaemonStatus,
} from "../../scripts/launch-desktop.mjs";

describe("resolveDesktopDaemonCleanupStep", () => {
  it("can target the product socket when a foreign daemon must be killed", () => {
    expect(
      resolveDesktopDaemonCleanupStep("/Users/test/projects/taugentic", {
        includeProductSocket: true,
        name: "daemon:force-cleanup",
      }),
    ).toEqual({
      name: "daemon:force-cleanup",
      command: [
        "node",
        resolve("/Users/test/projects/taugentic", "scripts/daemon-cleanup.mjs"),
        "--apply",
        "--include-product-socket",
      ],
      cwd: "/Users/test/projects/taugentic",
    });
  });
});

describe("resolveDesktopDevPreflightCommands", () => {
  it("stops the global daemon and cleans stale sockets before dev launch", () => {
    expect(resolveDesktopDevPreflightCommands("/Users/test/projects/taugentic")).toEqual([
      {
        name: "daemon:stop",
        command: [
          "cargo",
          "run",
          "--package",
          "ta-orchestrator",
          "--bin",
          "ta-daemon",
          "--",
          "__runtime-control-bootstrap",
          "stop",
        ],
        cwd: "/Users/test/projects/taugentic",
      },
      {
        name: "daemon:reset-local",
        command: [
          "cargo",
          "run",
          "--package",
          "ta-orchestrator",
          "--bin",
          "ta-daemon",
          "--",
          "__runtime-control-bootstrap",
          "reset-local",
        ],
        cwd: "/Users/test/projects/taugentic",
      },
      {
        name: "daemon:cleanup",
        command: [
          "node",
          resolve("/Users/test/projects/taugentic", "scripts/daemon-cleanup.mjs"),
          "--apply",
        ],
        cwd: "/Users/test/projects/taugentic",
      },
    ]);
  });
});

describe("resolveDaemonBootstrapCommand", () => {
  it("builds the canonical bootstrap command shape", () => {
    expect(resolveDaemonBootstrapCommand("snapshot")).toEqual([
      "cargo",
      "run",
      "--package",
      "ta-orchestrator",
      "--bin",
      "ta-daemon",
      "--",
      "__runtime-control-bootstrap",
      "snapshot",
    ]);
  });
});

describe("resolveElectronLaunchArguments", () => {
  it("keeps the default launch command when no debug env is configured", () => {
    expect(resolveElectronLaunchArguments({})).toEqual([
      "./node_modules/.bin/electron",
      "dist/index.js",
    ]);
  });

  it("adds the Electron agent debugging switches from env", () => {
    expect(
      resolveElectronLaunchArguments({
        TAUGENTIC_ELECTRON_REMOTE_DEBUGGING_PORT: "8315",
        TAUGENTIC_ELECTRON_INSPECT_PORT: "9229",
        TAUGENTIC_ELECTRON_ENABLE_LOGGING: "1",
        TMPDIR: "/var/folders/test/T/",
      }),
    ).toEqual([
      "./node_modules/.bin/electron",
      "dist/index.js",
      "--remote-debugging-port=8315",
      "--inspect=9229",
      "--enable-logging",
      "--log-file=/var/folders/test/T/my-electron.log",
    ]);
  });

  it("rejects invalid debug ports up front", () => {
    expect(() =>
      resolveElectronLaunchArguments({
        TAUGENTIC_ELECTRON_REMOTE_DEBUGGING_PORT: "not-a-port",
      }),
    ).toThrow(
      'TAUGENTIC_ELECTRON_REMOTE_DEBUGGING_PORT must be a numeric port, received "not-a-port"',
    );
  });
});

describe("isLaunchDesktopEntrypoint", () => {
  it("returns false when imported from tests instead of executed directly", () => {
    expect(
      isLaunchDesktopEntrypoint(
        ["/opt/homebrew/bin/node", "/tmp/other-script.mjs"],
        "file:///Users/kregenrek/projects/taugentic/apps/desktop/scripts/launch-desktop.mjs",
      ),
    ).toBe(false);
  });

  it("returns true for the direct script entrypoint path", () => {
    expect(
      isLaunchDesktopEntrypoint(
        [
          "/opt/homebrew/bin/node",
          "/Users/kregenrek/projects/taugentic/apps/desktop/scripts/launch-desktop.mjs",
        ],
        "file:///Users/kregenrek/projects/taugentic/apps/desktop/scripts/launch-desktop.mjs",
      ),
    ).toBe(true);
  });
});

describe("parseDesktopDevOrphanProcessIds", () => {
  it("returns only orphaned desktop dev processes from this repo scope", () => {
    const scopeDir = "/Users/test/projects/taugentic/apps/desktop";
    const processTable = [
      `101 1 /opt/homebrew/bin/node ${scopeDir}/node_modules/.pnpm/@voidzero-dev/vite-plus-core/dist/vite/node/cli.js dev --host 127.0.0.1 --port 1420`,
      `102 1 /opt/homebrew/bin/node ${scopeDir}/node_modules/.pnpm/@voidzero-dev/vite-plus-core/dist/vite/node/cli.js build --watch`,
      `103 77 /opt/homebrew/bin/node ${scopeDir}/node_modules/.pnpm/@voidzero-dev/vite-plus-core/dist/vite/node/cli.js dev --host 127.0.0.1 --port 1420`,
      `104 1 /opt/homebrew/bin/node /Users/test/projects/other/apps/desktop/node_modules/.pnpm/@voidzero-dev/vite-plus-core/dist/vite/node/cli.js dev --host 127.0.0.1 --port 1420`,
      `105 1 /Users/test/projects/taugentic/target/debug/ta-daemon`,
      `106 1 /Users/test/projects/taugentic/apps/desktop/node_modules/.pnpm/electron@41.2.0/node_modules/electron/dist/Electron.app/Contents/MacOS/Electron dist/index.js`,
    ].join("\n");

    expect(parseDesktopDevOrphanProcessIds(processTable, scopeDir)).toEqual([101, 102, 106]);
  });
});

describe("shouldForceCleanupForDaemonStatus", () => {
  it("forces cleanup only for foreign runtime snapshots", () => {
    expect(shouldForceCleanupForDaemonStatus({ actualMode: "foreign" })).toBe(true);
    expect(shouldForceCleanupForDaemonStatus({ actualMode: "local" })).toBe(false);
    expect(shouldForceCleanupForDaemonStatus({ actualMode: "stopped" })).toBe(false);
  });
});
