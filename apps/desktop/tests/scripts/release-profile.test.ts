import { describe, expect, it } from "vite-plus/test";

import {
  buildDesktopArtifactNameTemplate,
  getDesktopReleaseProfileConfig,
  parseDesktopReleaseProfile,
  resolveDesktopReleaseProfile,
} from "../../scripts/release-profile.mjs";

describe("parseDesktopReleaseProfile", () => {
  it("defaults to stable and rejects unknown profiles", () => {
    expect(parseDesktopReleaseProfile(undefined)).toBe("stable");
    expect(parseDesktopReleaseProfile("Nightly")).toBe("nightly");
    expect(() => parseDesktopReleaseProfile("beta")).toThrow(
      "unknown desktop release profile: beta; expected one of stable, nightly, mission-control",
    );
  });
});

describe("resolveDesktopReleaseProfile", () => {
  it("prefers argv, then env, then stable", () => {
    expect(resolveDesktopReleaseProfile(["--release-profile=mission-control"], {})).toBe(
      "mission-control",
    );
    expect(resolveDesktopReleaseProfile([], { TAUGENTIC_DESKTOP_RELEASE_PROFILE: "nightly" })).toBe(
      "nightly",
    );
    expect(resolveDesktopReleaseProfile([], {})).toBe("stable");
  });
});

describe("getDesktopReleaseProfileConfig", () => {
  it("returns install-safe product identity per profile", () => {
    expect(getDesktopReleaseProfileConfig("stable")).toEqual({
      appId: "app.taugentic.desktop",
      artifactStem: "taugentic-desktop",
      channel: "stable",
      packageName: "taugentic-desktop-app",
      productName: "Taugentic",
    });
    expect(getDesktopReleaseProfileConfig("nightly").appId).toBe("app.taugentic.desktop.nightly");
    expect(getDesktopReleaseProfileConfig("mission-control").productName).toBe(
      "Taugentic Mission Control",
    );
  });
});

describe("release artifact helpers", () => {
  it("derives deterministic artifact templates without a profile-owned publisher path", () => {
    expect(buildDesktopArtifactNameTemplate("nightly")).toBe(
      "taugentic-desktop-nightly-${version}-${os}-${arch}.${ext}",
    );
  });
});
