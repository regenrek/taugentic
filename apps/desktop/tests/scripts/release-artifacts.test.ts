import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";

import { afterEach, describe, expect, it } from "vite-plus/test";

import {
  buildReleaseArtifactManifest,
  formatAttestationSubjects,
  formatReleaseChecksums,
  isPrimaryReleaseArtifact,
  isRecognizedReleaseArtifact,
  writeReleaseArtifactsBundle,
} from "../../scripts/release-artifacts.mjs";

const tempDirs: string[] = [];

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map((dir) => rm(dir, { force: true, recursive: true })));
});

describe("isRecognizedReleaseArtifact", () => {
  it("accepts bundle members and ignores generated metadata files", () => {
    expect(isRecognizedReleaseArtifact("Taugentic-1.0.0.dmg")).toBe(true);
    expect(isRecognizedReleaseArtifact("latest-mac.yml")).toBe(true);
    expect(isRecognizedReleaseArtifact("release-manifest.json")).toBe(false);
    expect(isRecognizedReleaseArtifact("notes.txt")).toBe(false);
  });
});

describe("isPrimaryReleaseArtifact", () => {
  it("accepts installable artifacts and rejects metadata-only bundle members", () => {
    expect(isPrimaryReleaseArtifact("Taugentic-1.0.0.dmg")).toBe(true);
    expect(isPrimaryReleaseArtifact("Taugentic-1.0.0.zip")).toBe(true);
    expect(isPrimaryReleaseArtifact("latest-mac.yml")).toBe(false);
    expect(isPrimaryReleaseArtifact("Taugentic-1.0.0.dmg.blockmap")).toBe(false);
  });
});

describe("buildReleaseArtifactManifest", () => {
  it("collects installable artifacts plus their release metadata and emits repo-root-relative paths", async () => {
    const releaseDir = await createTempReleaseDir();
    await writeFile(path.join(releaseDir, "Taugentic.dmg"), "mac-release", "utf8");
    await writeFile(path.join(releaseDir, "latest-mac.yml"), "version: 1", "utf8");
    await writeFile(path.join(releaseDir, "release-manifest.json"), "{}", "utf8");
    await writeFile(path.join(releaseDir, "notes.txt"), "skip me", "utf8");
    await mkdir(path.join(releaseDir, "nested"), { recursive: true });
    await writeFile(
      path.join(releaseDir, "nested", "Taugentic Setup.exe"),
      "windows-release",
      "utf8",
    );

    const manifest = await buildReleaseArtifactManifest(releaseDir);

    expect(manifest.map((entry) => entry.path)).toEqual([
      "Taugentic.dmg",
      "latest-mac.yml",
      "nested/Taugentic Setup.exe",
    ]);
    expect(manifest.every((entry) => !path.isAbsolute(entry.workspacePath))).toBe(true);
    expect(manifest.some((entry) => entry.workspacePath.endsWith("/Taugentic.dmg"))).toBe(true);
    expect(
      manifest.some((entry) => entry.workspacePath.endsWith("/nested/Taugentic Setup.exe")),
    ).toBe(true);
  });

  it("fails fast when the release directory has no publishable artifacts", async () => {
    const releaseDir = await createTempReleaseDir();
    await writeFile(path.join(releaseDir, "README.txt"), "no artifacts", "utf8");

    await expect(buildReleaseArtifactManifest(releaseDir)).rejects.toThrow(
      `no release artifacts found in ${releaseDir}`,
    );
  });

  it("fails fast when the release directory contains only metadata-only bundle members", async () => {
    const releaseDir = await createTempReleaseDir();
    await writeFile(path.join(releaseDir, "latest-mac.yml"), "version: 1", "utf8");
    await writeFile(path.join(releaseDir, "latest.yml"), "version: 1", "utf8");
    await writeFile(path.join(releaseDir, "Taugentic-1.0.0.dmg.blockmap"), "blockmap", "utf8");

    await expect(buildReleaseArtifactManifest(releaseDir)).rejects.toThrow(
      `no installable release artifacts found in ${releaseDir}`,
    );
  });
});

describe("writeReleaseArtifactsBundle", () => {
  it("writes manifest, shasum-compatible checksums, and attestation subjects", async () => {
    const releaseDir = await createTempReleaseDir();
    await writeFile(path.join(releaseDir, "Taugentic.deb"), "linux-release", "utf8");

    const result = await writeReleaseArtifactsBundle({ releaseDir });
    const checksumContents = await readFile(result.checksumPath, "utf8");
    const subjectsContents = await readFile(result.subjectsPath, "utf8");
    const manifestContents = JSON.parse(await readFile(result.manifestPath, "utf8"));

    expect(checksumContents).toBe(formatReleaseChecksums(result.manifest));
    expect(checksumContents).toContain(" *");
    expect(subjectsContents).toBe(formatAttestationSubjects(result.manifest));
    expect(manifestContents).toEqual(result.manifest);
  });
});

async function createTempReleaseDir(): Promise<string> {
  const dir = await mkdtemp(path.join(tmpdir(), "taugentic-release-artifacts-"));
  tempDirs.push(dir);
  return dir;
}
