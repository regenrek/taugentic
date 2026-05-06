import { describe, expect, it } from "vite-plus/test";

import {
  assertReleaseSigningEnv,
  missingReleaseSigningEnv,
  requiredReleaseSigningEnv,
} from "../../scripts/release-env.mjs";

describe("requiredReleaseSigningEnv", () => {
  it("requires Apple and certificate env for darwin releases", () => {
    expect(requiredReleaseSigningEnv("darwin")).toEqual([
      "APPLE_APP_SPECIFIC_PASSWORD",
      "APPLE_ID",
      "APPLE_TEAM_ID",
      "CSC_KEY_PASSWORD",
      "CSC_LINK",
    ]);
  });

  it("requires certificate env for windows releases and nothing for linux", () => {
    expect(requiredReleaseSigningEnv("win32")).toEqual(["CSC_KEY_PASSWORD", "CSC_LINK"]);
    expect(requiredReleaseSigningEnv("linux")).toEqual([]);
  });
});

describe("missingReleaseSigningEnv", () => {
  it("reports only absent or blank signing env values", () => {
    expect(
      missingReleaseSigningEnv("darwin", {
        APPLE_APP_SPECIFIC_PASSWORD: "app-pass",
        APPLE_ID: "builds@example.com",
        APPLE_TEAM_ID: "TEAM123456",
        CSC_KEY_PASSWORD: "",
        CSC_LINK: "base64-cert",
      }),
    ).toEqual(["CSC_KEY_PASSWORD"]);
  });
});

describe("assertReleaseSigningEnv", () => {
  it("passes when all required env is present", () => {
    expect(() =>
      assertReleaseSigningEnv("win32", {
        CSC_KEY_PASSWORD: "secret",
        CSC_LINK: "base64-cert",
      }),
    ).not.toThrow();
  });

  it("fails fast with the exact missing env list", () => {
    expect(() => assertReleaseSigningEnv("darwin", {})).toThrow(
      "missing release signing env for darwin: APPLE_APP_SPECIFIC_PASSWORD, APPLE_ID, APPLE_TEAM_ID, CSC_KEY_PASSWORD, CSC_LINK",
    );
  });
});
