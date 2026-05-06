import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { desktopRootDir, parseArgvFlagValue, repoRootDir } from "./package-layout.mjs";
import { compareReleaseRelativePaths } from "./release-path-order.mjs";

const GENERATED_RELEASE_FILES = new Set([
  "attestation-subjects.txt",
  "release-manifest.json",
  "release-sha256.txt",
]);

const RELEASE_ARTIFACT_SUFFIXES = [
  ".AppImage",
  ".blockmap",
  ".deb",
  ".dmg",
  ".exe",
  ".pkg",
  ".rpm",
  ".tar.gz",
  ".yaml",
  ".yml",
  ".zip",
];

const PRIMARY_RELEASE_ARTIFACT_SUFFIXES = [
  ".AppImage",
  ".deb",
  ".dmg",
  ".exe",
  ".pkg",
  ".rpm",
  ".tar.gz",
  ".zip",
];

export function resolveReleaseDir(argv = process.argv.slice(2), env = process.env) {
  const explicitDir =
    parseArgvFlagValue(argv, "--release-dir") ?? env.TAUGENTIC_DESKTOP_RELEASE_DIR;
  return explicitDir ? path.resolve(explicitDir) : path.join(desktopRootDir, "release");
}

export function isGeneratedReleaseMetadataFile(relativePath) {
  return GENERATED_RELEASE_FILES.has(path.basename(relativePath));
}

export function isRecognizedReleaseArtifact(relativePath) {
  if (isGeneratedReleaseMetadataFile(relativePath)) {
    return false;
  }
  const fileName = path.basename(relativePath);
  return RELEASE_ARTIFACT_SUFFIXES.some((suffix) => fileName.endsWith(suffix));
}

export function isPrimaryReleaseArtifact(relativePath) {
  if (!isRecognizedReleaseArtifact(relativePath)) {
    return false;
  }
  const fileName = path.basename(relativePath);
  return PRIMARY_RELEASE_ARTIFACT_SUFFIXES.some((suffix) => fileName.endsWith(suffix));
}

export async function collectReleaseArtifactPaths(releaseDir) {
  const relativePaths = [];
  await walkReleaseDir(releaseDir, releaseDir, relativePaths);
  const artifacts = relativePaths
    .filter(isRecognizedReleaseArtifact)
    .sort(compareReleaseRelativePaths);
  if (artifacts.length === 0) {
    throw new Error(`no release artifacts found in ${releaseDir}`);
  }
  if (!artifacts.some(isPrimaryReleaseArtifact)) {
    throw new Error(`no installable release artifacts found in ${releaseDir}`);
  }
  return artifacts;
}

export async function buildReleaseArtifactManifest(releaseDir) {
  const artifactPaths = await collectReleaseArtifactPaths(releaseDir);
  return await Promise.all(
    artifactPaths.map(async (relativePath) => {
      const absolutePath = path.join(releaseDir, relativePath);
      const fileBuffer = await readFile(absolutePath);
      const fileStats = await stat(absolutePath);
      return {
        path: relativePath,
        sha256: createHash("sha256").update(fileBuffer).digest("hex"),
        sizeBytes: fileStats.size,
        workspacePath: relativeToRepoRoot(absolutePath),
      };
    }),
  );
}

export function formatReleaseChecksums(manifest) {
  return `${manifest.map((entry) => `${entry.sha256} *${entry.workspacePath}`).join("\n")}\n`;
}

export function formatAttestationSubjects(manifest) {
  return `${manifest.map((entry) => entry.workspacePath).join("\n")}\n`;
}

export async function writeReleaseArtifactsBundle(options) {
  const releaseDir = options.releaseDir;
  const manifest = await buildReleaseArtifactManifest(releaseDir);
  const manifestPath = options.manifestPath ?? path.join(releaseDir, "release-manifest.json");
  const checksumPath = options.checksumPath ?? path.join(releaseDir, "release-sha256.txt");
  const subjectsPath = options.subjectsPath ?? path.join(releaseDir, "attestation-subjects.txt");

  await mkdir(path.dirname(manifestPath), { recursive: true });
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  await writeFile(checksumPath, formatReleaseChecksums(manifest), "utf8");
  await writeFile(subjectsPath, formatAttestationSubjects(manifest), "utf8");

  return {
    checksumPath,
    manifest,
    manifestPath,
    subjectsPath,
  };
}

async function walkReleaseDir(rootDir, currentDir, relativePaths) {
  const entries = await readdir(currentDir, { withFileTypes: true });
  for (const entry of entries) {
    const entryAbsolutePath = path.join(currentDir, entry.name);
    if (entry.isDirectory()) {
      await walkReleaseDir(rootDir, entryAbsolutePath, relativePaths);
      continue;
    }
    if (!entry.isFile()) {
      continue;
    }
    relativePaths.push(relativeToPosix(rootDir, entryAbsolutePath));
  }
}

function relativeToRepoRoot(absolutePath) {
  return relativeToPosix(repoRootDir, absolutePath);
}

function relativeToPosix(baseDir, targetPath) {
  return path.relative(baseDir, targetPath).split(path.sep).join("/");
}

async function main() {
  const argv = process.argv.slice(2);
  const releaseDir = resolveReleaseDir(argv, process.env);
  const manifestPath = parseArgvFlagValue(argv, "--manifest");
  const checksumPath = parseArgvFlagValue(argv, "--checksums");
  const subjectsPath = parseArgvFlagValue(argv, "--subjects");
  const result = await writeReleaseArtifactsBundle({
    releaseDir,
    manifestPath: manifestPath ? path.resolve(manifestPath) : undefined,
    checksumPath: checksumPath ? path.resolve(checksumPath) : undefined,
    subjectsPath: subjectsPath ? path.resolve(subjectsPath) : undefined,
  });

  console.log(`release manifest: ${result.manifestPath}`);
  console.log(`release checksums: ${result.checksumPath}`);
  console.log(`attestation subjects: ${result.subjectsPath}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  await main();
}
