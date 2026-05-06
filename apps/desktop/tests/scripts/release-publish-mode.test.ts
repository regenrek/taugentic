import { describe, expect, it } from "vite-plus/test";

import {
  extractDesktopReleaseTag,
  isDesktopReleaseTagRef,
  resolveDesktopReleasePublishMode,
  resolveDesktopReleasePublishModeForRef,
} from "../../scripts/release-publish-mode.mjs";
import { readDesktopMainPackageVersion } from "../../scripts/release-version.mjs";

describe("release publish mode", () => {
  it("treats only v-tag refs as publishable", () => {
    expect(isDesktopReleaseTagRef("refs/tags/v1.2.3")).toBe(true);
    expect(isDesktopReleaseTagRef("refs/heads/main")).toBe(false);
    expect(extractDesktopReleaseTag("refs/tags/v1.2.3")).toBe("v1.2.3");
    expect(extractDesktopReleaseTag("refs/heads/main")).toBe(null);
  });

  it("forces non-tag refs to publish never", () => {
    expect(
      resolveDesktopReleasePublishModeForRef("refs/heads/release-hardening", "1.2.3", "stable"),
    ).toBe("never");
  });

  it("allows publish always only for matching stable tag refs", () => {
    expect(resolveDesktopReleasePublishModeForRef("refs/tags/v1.2.3", "1.2.3", "stable")).toBe(
      "always",
    );
    expect(resolveDesktopReleasePublishModeForRef("refs/tags/v1.2.3", "1.2.3", "nightly")).toBe(
      "never",
    );
    expect(
      resolveDesktopReleasePublishModeForRef("refs/tags/v1.2.3", "1.2.3", "mission-control"),
    ).toBe("never");
    expect(() =>
      resolveDesktopReleasePublishModeForRef("refs/tags/v1.2.4", "1.2.3", "stable"),
    ).toThrow("release tag v1.2.4 does not match desktop package version 1.2.3");
  });

  it("reads release profile from env and only durably publishes stable tags", async () => {
    const packageVersion = await readDesktopMainPackageVersion();
    const tagRef = `refs/tags/v${packageVersion}`;

    await expect(resolveDesktopReleasePublishMode([], { GITHUB_REF: tagRef })).resolves.toBe(
      "always",
    );
    await expect(
      resolveDesktopReleasePublishMode([], {
        GITHUB_REF: tagRef,
        TAUGENTIC_DESKTOP_RELEASE_PROFILE: "stable",
      }),
    ).resolves.toBe("always");
    await expect(
      resolveDesktopReleasePublishMode([], {
        GITHUB_REF: tagRef,
        TAUGENTIC_DESKTOP_RELEASE_PROFILE: "mission-control",
      }),
    ).resolves.toBe("never");
  });
});
