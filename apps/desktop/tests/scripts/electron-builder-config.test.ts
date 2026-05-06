import path from "node:path";

import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import {
  desktopRootDir,
  stagedAppDir,
  stagedResourcesBinDir,
} from "../../scripts/package-layout.mjs";

const builderConfigModuleUrl = new URL("../../electron-builder.config.mjs", import.meta.url).href;
let importSequence = 0;

describe("electron-builder release config", () => {
  const originalArgv = [...process.argv];
  const originalReleaseProfile = process.env.TAUGENTIC_DESKTOP_RELEASE_PROFILE;

  beforeEach(() => {
    vi.resetModules();
    process.argv = ["node", "electron-builder.config.mjs"];
  });

  afterEach(() => {
    process.argv = [...originalArgv];
    if (originalReleaseProfile == null) {
      delete process.env.TAUGENTIC_DESKTOP_RELEASE_PROFILE;
    } else {
      process.env.TAUGENTIC_DESKTOP_RELEASE_PROFILE = originalReleaseProfile;
    }
  });

  it("defaults to stable packaged identity without modeling a publisher", async () => {
    delete process.env.TAUGENTIC_DESKTOP_RELEASE_PROFILE;

    const config = await loadBuilderConfig();

    expect(config.appId).toBe("app.taugentic.desktop");
    expect(config.productName).toBe("Taugentic");
    expect(config.artifactName).toBe("taugentic-desktop-${version}-${os}-${arch}.${ext}");
    expect(config.publish).toBeUndefined();
  });

  it("switches packaged identity per release profile while keeping publish undefined", async () => {
    process.env.TAUGENTIC_DESKTOP_RELEASE_PROFILE = "mission-control";

    const config = await loadBuilderConfig();

    expect(config.appId).toBe("app.taugentic.desktop.mission-control");
    expect(config.productName).toBe("Taugentic Mission Control");
    expect(config.artifactName).toBe("taugentic-mission-control-${version}-${os}-${arch}.${ext}");
    expect(config.publish).toBeUndefined();
  });

  it("packages the staged app tree and staged daemon binary through the canonical builder paths for every release profile", async () => {
    for (const releaseProfile of ["stable", "nightly", "mission-control"] as const) {
      process.env.TAUGENTIC_DESKTOP_RELEASE_PROFILE = releaseProfile;

      const config = await loadBuilderConfig();

      expect(config.directories).toEqual({
        app: path.relative(desktopRootDir, stagedAppDir),
        output: "release",
      });
      expect(config.files).toEqual(["package.json", "node_modules/**/*", "packages/**/*"]);
      expect(config.extraResources).toEqual([
        {
          filter: ["ta-daemon", "ta-daemon.exe"],
          from: path.relative(desktopRootDir, stagedResourcesBinDir),
          to: "bin",
        },
      ]);
    }
  });
});

async function loadBuilderConfig() {
  return (await import(`${builderConfigModuleUrl}?case=${importSequence++}`)).default;
}
