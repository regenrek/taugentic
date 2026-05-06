import { describe, expect, it } from "vite-plus/test";

import { buildDesktopArtifactNameTemplate } from "../../scripts/release-profile.mjs";
import {
  assertStageTargetPlatformConfiguration,
  buildStageAppPackageManifest,
  daemonBinaryFileNameForPlatform,
  getDesktopReleaseProfileConfig,
  normalizeStageTargetPlatform,
  parseArgvFlagValue,
  resolveDesktopReleaseProfile,
  resolveStageCargoTargetTriple,
  resolveStageTargetPlatform,
  stageCopyPlan,
  stagedAppMainEntry,
} from "../../scripts/package-layout.mjs";

describe("buildStageAppPackageManifest", () => {
  it("produces a minimal packaged app manifest anchored at the staged main entry", () => {
    expect(buildStageAppPackageManifest({ version: "0.2.3" })).toEqual({
      name: "taugentic-desktop-app",
      author: "Taugentic",
      description: "Taugentic desktop runtime shell",
      private: true,
      type: "module",
      version: "0.2.3",
      main: stagedAppMainEntry,
      dependencies: {
        "@taugentic/desktop-shared": "file:./packages/shared",
      },
    });
  });

  it("keeps release-profile identity in the dedicated profile owner", () => {
    expect(getDesktopReleaseProfileConfig("mission-control")).toMatchObject({
      packageName: "taugentic-mission-control-app",
      productName: "Taugentic Mission Control",
    });
  });

  it("keeps staged manifest and packaged artifact identity aligned for every release profile", () => {
    const profiles = ["stable", "nightly", "mission-control"] as const;
    const seenIdentities = new Set<string>();

    for (const profile of profiles) {
      const config = getDesktopReleaseProfileConfig(profile);
      const manifest = buildStageAppPackageManifest({ releaseProfile: profile, version: "1.2.3" });

      expect(manifest.name).toBe(config.packageName);
      expect(manifest.description).toBe(`${config.productName} desktop runtime shell`);
      expect(buildDesktopArtifactNameTemplate(profile)).toContain(config.artifactStem);

      const identityKey = JSON.stringify({
        appId: config.appId,
        artifactStem: config.artifactStem,
        channel: config.channel,
        packageName: config.packageName,
        productName: config.productName,
      });
      expect(seenIdentities.has(identityKey)).toBe(false);
      seenIdentities.add(identityKey);
    }
  });
});

describe("daemonBinaryFileNameForPlatform", () => {
  it("matches the packaged daemon binary name convention", () => {
    expect(daemonBinaryFileNameForPlatform("darwin")).toBe("ta-daemon");
    expect(daemonBinaryFileNameForPlatform("linux")).toBe("ta-daemon");
    expect(daemonBinaryFileNameForPlatform("win32")).toBe("ta-daemon.exe");
  });
});

describe("stageCopyPlan", () => {
  it("stages the compiled desktop packages and shared manifest", () => {
    const stagedTargets = stageCopyPlan().map((entry) => entry.to);

    expect(stagedTargets).toContainEqual(expect.stringContaining("packages/main/dist"));
    expect(stagedTargets).toContainEqual(expect.stringContaining("packages/preload/dist"));
    expect(stagedTargets).toContainEqual(expect.stringContaining("packages/renderer/dist"));
    expect(stagedTargets).toContainEqual(expect.stringContaining("packages/shared/dist"));
    expect(stagedTargets).toContainEqual(expect.stringContaining("packages/shared/package.json"));
  });
});

describe("parseArgvFlagValue", () => {
  it("reads --flag=value and spaced --flag value", () => {
    expect(parseArgvFlagValue(["--platform=linux"], "--platform")).toBe("linux");
    expect(parseArgvFlagValue(["--platform", "linux"], "--platform")).toBe("linux");
    expect(
      parseArgvFlagValue(["--cargo-target", "aarch64-unknown-linux-gnu"], "--cargo-target"),
    ).toBe("aarch64-unknown-linux-gnu");
    expect(parseArgvFlagValue(["other"], "--platform")).toBeNull();
  });
});

describe("normalizeStageTargetPlatform", () => {
  it("maps common aliases to node platforms", () => {
    expect(normalizeStageTargetPlatform("macos", "linux")).toBe("darwin");
    expect(normalizeStageTargetPlatform("windows", "darwin")).toBe("win32");
    expect(normalizeStageTargetPlatform("unknown", "linux")).toBe("linux");
  });
});

describe("resolveStageTargetPlatform", () => {
  it("prefers argv, then TAUGENTIC_DESKTOP_PACKAGE_PLATFORM, then npm_config_platform, then fallback", () => {
    expect(resolveStageTargetPlatform(["--platform=linux"], {}, "darwin")).toBe("linux");
    expect(
      resolveStageTargetPlatform(
        ["--platform", "linux"],
        { TAUGENTIC_DESKTOP_PACKAGE_PLATFORM: "win32", npm_config_platform: "darwin" },
        "darwin",
      ),
    ).toBe("linux");
    expect(
      resolveStageTargetPlatform([], { TAUGENTIC_DESKTOP_PACKAGE_PLATFORM: "win32" }, "darwin"),
    ).toBe("win32");
    expect(resolveStageTargetPlatform([], { npm_config_platform: "linux" }, "darwin")).toBe(
      "linux",
    );
    expect(resolveStageTargetPlatform([], {}, "darwin")).toBe("darwin");
  });
});

describe("resolveStageCargoTargetTriple", () => {
  it("reads --cargo-target= then TAUGENTIC_DESKTOP_CARGO_TARGET", () => {
    expect(resolveStageCargoTargetTriple(["--cargo-target=x86_64-pc-windows-msvc"], {})).toBe(
      "x86_64-pc-windows-msvc",
    );
    expect(
      resolveStageCargoTargetTriple(["--cargo-target", "x86_64-unknown-linux-gnu"], {
        TAUGENTIC_DESKTOP_CARGO_TARGET: "aarch64-apple-darwin",
      }),
    ).toBe("x86_64-unknown-linux-gnu");
    expect(
      resolveStageCargoTargetTriple([], { TAUGENTIC_DESKTOP_CARGO_TARGET: "aarch64-apple-darwin" }),
    ).toBe("aarch64-apple-darwin");
    expect(resolveStageCargoTargetTriple([], {})).toBeNull();
  });
});

describe("resolveDesktopReleaseProfile", () => {
  it("reads --release-profile then TAUGENTIC_DESKTOP_RELEASE_PROFILE", () => {
    expect(resolveDesktopReleaseProfile(["--release-profile=nightly"], {})).toBe("nightly");
    expect(
      resolveDesktopReleaseProfile([], { TAUGENTIC_DESKTOP_RELEASE_PROFILE: "mission-control" }),
    ).toBe("mission-control");
    expect(resolveDesktopReleaseProfile([], {})).toBe("stable");
  });
});

describe("getDesktopReleaseProfileConfig", () => {
  it("exposes install-safe identity per release profile", () => {
    expect(getDesktopReleaseProfileConfig("stable").appId).toBe("app.taugentic.desktop");
    expect(getDesktopReleaseProfileConfig("nightly").productName).toBe("Taugentic Nightly");
    expect(getDesktopReleaseProfileConfig("mission-control").channel).toBe("mission-control");
  });
});

describe("assertStageTargetPlatformConfiguration", () => {
  it("allows same-platform staging without a cargo target triple", () => {
    expect(() => assertStageTargetPlatformConfiguration("darwin", "darwin", null)).not.toThrow();
  });

  it("allows cross-platform staging with an explicit cargo target triple", () => {
    expect(() =>
      assertStageTargetPlatformConfiguration("linux", "darwin", "x86_64-unknown-linux-gnu"),
    ).not.toThrow();
  });

  it("fails fast for cross-platform staging without a cargo target triple", () => {
    expect(() => assertStageTargetPlatformConfiguration("linux", "darwin", null)).toThrow(
      "cross-platform desktop packaging from darwin to linux requires --cargo-target or TAUGENTIC_DESKTOP_CARGO_TARGET",
    );
  });
});
