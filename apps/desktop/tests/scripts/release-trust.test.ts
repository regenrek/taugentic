import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";

import { afterEach, describe, expect, it } from "vite-plus/test";

import {
  assertAuthenticodeStatus,
  assertMacCodesignOutput,
  assertMacStaplerOutput,
  collectTrustSubjects,
  isMacTrustSubject,
  isWindowsTrustSubject,
} from "../../scripts/release-trust.mjs";

const tempDirs: string[] = [];

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map((dir) => rm(dir, { force: true, recursive: true })));
});

describe("release trust subject selection", () => {
  it("recognizes mac app bundles and dmg/pkg artifacts", () => {
    expect(isMacTrustSubject("mac-arm64/Taugentic.app")).toBe(true);
    expect(isMacTrustSubject("Taugentic.dmg")).toBe(true);
    expect(isMacTrustSubject("Taugentic.exe")).toBe(false);
  });

  it("recognizes windows executable artifacts", () => {
    expect(isWindowsTrustSubject("Taugentic Setup.exe")).toBe(true);
    expect(isWindowsTrustSubject("installer.msi")).toBe(true);
    expect(isWindowsTrustSubject("Taugentic.dmg")).toBe(false);
  });
});

describe("release trust output assertions", () => {
  it("rejects ad-hoc mac signatures and missing team ids", () => {
    expect(() =>
      assertMacCodesignOutput("Signature=adhoc\nTeamIdentifier=TEAM123\n", "Taugentic.app"),
    ).toThrow("release artifact is ad-hoc signed: Taugentic.app");
    expect(() =>
      assertMacCodesignOutput("Signature=Developer ID\nTeamIdentifier=not set\n", "Taugentic.app"),
    ).toThrow("release artifact is missing a TeamIdentifier: Taugentic.app");
  });

  it("rejects unstapled mac artifacts", () => {
    expect(() =>
      assertMacStaplerOutput(
        "Taugentic.app does not have a ticket stapled to it.\n",
        "Taugentic.app",
      ),
    ).toThrow("release artifact is not stapled: Taugentic.app");
  });

  it("rejects non-valid Authenticode status", () => {
    expect(() => assertAuthenticodeStatus("UnknownError", "Taugentic.exe")).toThrow(
      "release artifact is not Authenticode-valid: Taugentic.exe (UnknownError)",
    );
  });
});

describe("collectTrustSubjects", () => {
  it("collects app bundles on mac and executable installers on windows", async () => {
    const releaseDir = await createTempReleaseDir();
    await mkdir(path.join(releaseDir, "mac-arm64", "Taugentic.app"), { recursive: true });
    await writeFile(path.join(releaseDir, "mac-arm64", "Taugentic.app", "Contents"), "", "utf8");
    await writeFile(path.join(releaseDir, "Taugentic.dmg"), "", "utf8");
    await writeFile(path.join(releaseDir, "Taugentic Setup.exe"), "", "utf8");

    await expect(collectTrustSubjects(releaseDir, "darwin")).resolves.toEqual([
      "Taugentic.dmg",
      "mac-arm64/Taugentic.app",
    ]);
    await expect(collectTrustSubjects(releaseDir, "win32")).resolves.toEqual([
      "Taugentic Setup.exe",
    ]);
  });

  it("fails fast when no trust-checkable artifact exists for the target platform", async () => {
    const releaseDir = await createTempReleaseDir();
    await writeFile(path.join(releaseDir, "README.txt"), "noop", "utf8");

    await expect(collectTrustSubjects(releaseDir, "darwin")).rejects.toThrow(
      `no trust-checkable release artifacts found in ${releaseDir} for darwin`,
    );
  });
});

async function createTempReleaseDir(): Promise<string> {
  const dir = await mkdtemp(path.join(tmpdir(), "taugentic-release-trust-"));
  tempDirs.push(dir);
  return dir;
}
