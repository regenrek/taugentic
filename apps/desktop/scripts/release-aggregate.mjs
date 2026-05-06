import { copyFile, mkdir, readdir, rm } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { parseArgvFlagValue } from "./argv-flag.mjs";
import { compareReleaseRelativePaths } from "./release-path-order.mjs";
import {
  isGeneratedReleaseMetadataFile,
  isRecognizedReleaseArtifact,
  resolveReleaseDir,
} from "./release-artifacts.mjs";

const DOWNLOAD_RELEASE_PREFIX = "apps/desktop/release/";

export function resolveReleaseAggregateInputDir(argv = process.argv.slice(2), env = process.env) {
  const explicit =
    parseArgvFlagValue(argv, "--input-dir") ?? env.TAUGENTIC_DESKTOP_RELEASE_INPUT_DIR;
  if (!explicit) {
    throw new Error(
      "missing aggregated release input dir; pass --input-dir or TAUGENTIC_DESKTOP_RELEASE_INPUT_DIR",
    );
  }
  return path.resolve(explicit);
}

export function normalizeAggregatedReleaseRelativePath(relativePath) {
  const normalized = relativePath.split(path.sep).join("/");
  if (normalized.startsWith(DOWNLOAD_RELEASE_PREFIX)) {
    return normalized.slice(DOWNLOAD_RELEASE_PREFIX.length);
  }
  return normalized;
}

export async function aggregateDownloadedReleaseArtifacts({ inputDir, outputDir }) {
  const seenAssetNames = new Map();
  const seenRelativePaths = new Map();
  const aggregatedPaths = [];

  await rm(outputDir, { force: true, recursive: true });
  await mkdir(outputDir, { recursive: true });

  const entries = await readdir(inputDir, { withFileTypes: true });
  for (const entry of entries) {
    const absolutePath = path.join(inputDir, entry.name);
    if (entry.isDirectory()) {
      await walkDownloadedArtifactRoot({
        aggregatedPaths,
        artifactRootDir: absolutePath,
        currentDir: absolutePath,
        outputDir,
        seenAssetNames,
        seenRelativePaths,
      });
      continue;
    }
    if (entry.isFile()) {
      await copyAggregatedArtifact({
        absoluteSourcePath: absolutePath,
        aggregatedPaths,
        outputDir,
        rawRelativePath: entry.name,
        seenAssetNames,
        seenRelativePaths,
      });
    }
  }

  if (aggregatedPaths.length === 0) {
    throw new Error(`no aggregated release artifacts found in ${inputDir}`);
  }

  return aggregatedPaths.sort(compareReleaseRelativePaths);
}

async function walkDownloadedArtifactRoot({
  aggregatedPaths,
  artifactRootDir,
  currentDir,
  outputDir,
  seenAssetNames,
  seenRelativePaths,
}) {
  const entries = await readdir(currentDir, { withFileTypes: true });
  for (const entry of entries) {
    const absolutePath = path.join(currentDir, entry.name);
    if (entry.isDirectory()) {
      await walkDownloadedArtifactRoot({
        aggregatedPaths,
        artifactRootDir,
        currentDir: absolutePath,
        outputDir,
        seenAssetNames,
        seenRelativePaths,
      });
      continue;
    }
    if (!entry.isFile()) {
      continue;
    }
    await copyAggregatedArtifact({
      absoluteSourcePath: absolutePath,
      aggregatedPaths,
      outputDir,
      rawRelativePath: path.relative(artifactRootDir, absolutePath),
      seenAssetNames,
      seenRelativePaths,
    });
  }
}

async function copyAggregatedArtifact({
  absoluteSourcePath,
  aggregatedPaths,
  outputDir,
  rawRelativePath,
  seenAssetNames,
  seenRelativePaths,
}) {
  const releaseRelativePath = normalizeAggregatedReleaseRelativePath(rawRelativePath);
  if (releaseRelativePath.length === 0 || isGeneratedReleaseMetadataFile(releaseRelativePath)) {
    return;
  }
  if (!isRecognizedReleaseArtifact(releaseRelativePath)) {
    throw new Error(
      `unexpected aggregated release file: ${releaseRelativePath} from ${absoluteSourcePath}`,
    );
  }

  const previousPath = seenRelativePaths.get(releaseRelativePath);
  if (previousPath) {
    throw new Error(
      `duplicate aggregated release artifact path: ${releaseRelativePath} from ${previousPath} and ${absoluteSourcePath}`,
    );
  }

  const assetName = path.basename(releaseRelativePath);
  const previousAssetSource = seenAssetNames.get(assetName);
  if (previousAssetSource) {
    throw new Error(
      `duplicate aggregated release asset name: ${assetName} from ${previousAssetSource} and ${absoluteSourcePath}`,
    );
  }

  const absoluteOutputPath = path.join(outputDir, releaseRelativePath);
  await mkdir(path.dirname(absoluteOutputPath), { recursive: true });
  await copyFile(absoluteSourcePath, absoluteOutputPath);

  seenRelativePaths.set(releaseRelativePath, absoluteSourcePath);
  seenAssetNames.set(assetName, absoluteSourcePath);
  aggregatedPaths.push(releaseRelativePath);
}

async function main() {
  const argv = process.argv.slice(2);
  const inputDir = resolveReleaseAggregateInputDir(argv, process.env);
  const outputDir = resolveReleaseDir(argv, process.env);
  const aggregatedPaths = await aggregateDownloadedReleaseArtifacts({ inputDir, outputDir });
  console.log(`aggregated ${aggregatedPaths.length} release artifacts into ${outputDir}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  await main();
}
