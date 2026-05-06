import { mkdtemp, mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";

import { afterEach, describe, expect, it } from "vite-plus/test";

import {
  aggregateDownloadedReleaseArtifacts,
  normalizeAggregatedReleaseRelativePath,
} from "../../scripts/release-aggregate.mjs";

const tempDirs: string[] = [];

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map((dir) => rm(dir, { force: true, recursive: true })));
});

describe("normalizeAggregatedReleaseRelativePath", () => {
  it("strips the downloaded artifact release prefix when present", () => {
    expect(
      normalizeAggregatedReleaseRelativePath("apps/desktop/release/nested/Taugentic Setup.exe"),
    ).toBe("nested/Taugentic Setup.exe");
    expect(normalizeAggregatedReleaseRelativePath("Taugentic.dmg")).toBe("Taugentic.dmg");
  });
});

describe("aggregateDownloadedReleaseArtifacts", () => {
  it("collects packaged artifacts from downloaded workflow artifact roots and skips generated metadata", async () => {
    const inputDir = await createTempDir("taugentic-release-aggregate-input-");
    const outputDir = await createTempDir("taugentic-release-aggregate-output-");

    await writeDownloadedArtifact(
      inputDir,
      "desktop-release-stable-darwin",
      "apps/desktop/release/Taugentic.dmg",
      "darwin",
    );
    await writeDownloadedArtifact(
      inputDir,
      "desktop-release-stable-linux",
      "nested/taugentic-desktop-1.2.3-linux-x64.deb",
      "linux",
    );
    await writeDownloadedArtifact(
      inputDir,
      "desktop-release-stable-darwin",
      "apps/desktop/release/release-manifest.json",
      "{}",
    );

    await expect(
      aggregateDownloadedReleaseArtifacts({
        inputDir,
        outputDir,
      }),
    ).resolves.toEqual(["Taugentic.dmg", "nested/taugentic-desktop-1.2.3-linux-x64.deb"]);

    expect(await collectRelativeFiles(outputDir)).toEqual([
      "Taugentic.dmg",
      "nested/taugentic-desktop-1.2.3-linux-x64.deb",
    ]);
    await expect(readFile(path.join(outputDir, "Taugentic.dmg"), "utf8")).resolves.toBe("darwin");
  });

  it("fails fast on colliding release asset basenames across artifact roots", async () => {
    const inputDir = await createTempDir("taugentic-release-aggregate-collision-input-");
    const outputDir = await createTempDir("taugentic-release-aggregate-collision-output-");

    await writeDownloadedArtifact(
      inputDir,
      "desktop-release-stable-darwin",
      "apps/desktop/release/Taugentic.dmg",
      "darwin",
    );
    await writeDownloadedArtifact(
      inputDir,
      "desktop-release-stable-linux",
      "nested/Taugentic.dmg",
      "linux",
    );

    await expect(
      aggregateDownloadedReleaseArtifacts({
        inputDir,
        outputDir,
      }),
    ).rejects.toThrow("duplicate aggregated release asset name: Taugentic.dmg");
  });

  it("fails fast when a downloaded artifact root contains non-release files", async () => {
    const inputDir = await createTempDir("taugentic-release-aggregate-unknown-input-");
    const outputDir = await createTempDir("taugentic-release-aggregate-unknown-output-");

    await writeDownloadedArtifact(
      inputDir,
      "desktop-release-stable-darwin",
      "apps/desktop/release/Taugentic.dmg",
      "darwin",
    );
    await writeDownloadedArtifact(
      inputDir,
      "desktop-release-stable-darwin",
      "apps/desktop/release/README.txt",
      "debug stray",
    );

    await expect(
      aggregateDownloadedReleaseArtifacts({
        inputDir,
        outputDir,
      }),
    ).rejects.toThrow("unexpected aggregated release file: README.txt");
  });
});

async function createTempDir(prefix: string): Promise<string> {
  const dir = await mkdtemp(path.join(tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

async function writeDownloadedArtifact(
  inputDir: string,
  artifactRootName: string,
  relativePath: string,
  contents: string,
) {
  const absolutePath = path.join(inputDir, artifactRootName, relativePath);
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, contents, "utf8");
}

async function collectRelativeFiles(rootDir: string, currentDir = rootDir): Promise<string[]> {
  const entries = await readdir(currentDir, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const absolutePath = path.join(currentDir, entry.name);
      if (entry.isDirectory()) {
        return await collectRelativeFiles(rootDir, absolutePath);
      }
      return [
        absolutePath
          .slice(rootDir.length + 1)
          .split(path.sep)
          .join("/"),
      ];
    }),
  );
  return files.flat().sort();
}
