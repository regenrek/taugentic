import { describe, expect, it } from "vite-plus/test";

import {
  assertReleaseTagMatchesPackageVersion,
  normalizeReleaseTagVersion,
} from "../../scripts/release-version.mjs";

describe("normalizeReleaseTagVersion", () => {
  it("normalizes a leading v and tolerates empty input", () => {
    expect(normalizeReleaseTagVersion("v1.2.3")).toBe("1.2.3");
    expect(normalizeReleaseTagVersion("1.2.3")).toBe("1.2.3");
    expect(normalizeReleaseTagVersion("")).toBeNull();
    expect(normalizeReleaseTagVersion(undefined)).toBeNull();
  });
});

describe("assertReleaseTagMatchesPackageVersion", () => {
  it("passes when the tag version matches the packaged desktop version", () => {
    expect(() => assertReleaseTagMatchesPackageVersion("v0.0.1", "0.0.1")).not.toThrow();
  });

  it("fails fast on tag/package mismatch", () => {
    expect(() => assertReleaseTagMatchesPackageVersion("v1.2.3", "0.0.1")).toThrow(
      "release tag v1.2.3 does not match desktop package version 0.0.1",
    );
  });
});
