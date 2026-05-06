import { readdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

import { parseArgvFlagValue } from "./argv-flag.mjs";
import { desktopRootDir } from "./package-layout.mjs";
import { compareReleaseRelativePaths } from "./release-path-order.mjs";

const MAC_STAPLER_SUFFIXES = [".app", ".dmg", ".pkg"];
const WINDOWS_SIGNATURE_SUFFIXES = [".exe", ".msi"];

export function resolveTrustGatePlatform(argv = process.argv.slice(2), env = process.env) {
  const explicit = parseArgvFlagValue(argv, "--platform") ?? env.TAUGENTIC_DESKTOP_RELEASE_PLATFORM;
  if (explicit) {
    return explicit.trim().toLowerCase();
  }
  return process.platform;
}

export function resolveTrustGateReleaseDir(argv = process.argv.slice(2), env = process.env) {
  const explicit = parseArgvFlagValue(argv, "--release-dir") ?? env.TAUGENTIC_DESKTOP_RELEASE_DIR;
  return explicit ? path.resolve(explicit) : path.join(desktopRootDir, "release");
}

export function isMacTrustSubject(relativePath) {
  return MAC_STAPLER_SUFFIXES.some((suffix) => relativePath.endsWith(suffix));
}

export function isWindowsTrustSubject(relativePath) {
  return WINDOWS_SIGNATURE_SUFFIXES.some((suffix) => relativePath.endsWith(suffix));
}

export function assertMacCodesignOutput(output, subjectPath) {
  if (output.includes("Signature=adhoc")) {
    throw new Error(`release artifact is ad-hoc signed: ${subjectPath}`);
  }
  if (output.includes("TeamIdentifier=not set")) {
    throw new Error(`release artifact is missing a TeamIdentifier: ${subjectPath}`);
  }
}

export function assertMacStaplerOutput(output, subjectPath) {
  if (/does not have a ticket stapled to it/u.test(output)) {
    throw new Error(`release artifact is not stapled: ${subjectPath}`);
  }
}

export function assertAuthenticodeStatus(output, subjectPath) {
  if (output.trim() !== "Valid") {
    throw new Error(
      `release artifact is not Authenticode-valid: ${subjectPath} (${output.trim()})`,
    );
  }
}

export async function collectTrustSubjects(releaseDir, platform) {
  const subjects = [];
  await walkReleaseTree(releaseDir, releaseDir, platform, subjects);
  const filtered =
    platform === "darwin"
      ? subjects.filter(isMacTrustSubject)
      : platform === "win32"
        ? subjects.filter(isWindowsTrustSubject)
        : [];
  if (platform === "darwin" || platform === "win32") {
    if (filtered.length === 0) {
      throw new Error(
        `no trust-checkable release artifacts found in ${releaseDir} for ${platform}`,
      );
    }
  }
  return filtered.sort(compareReleaseRelativePaths);
}

export async function verifyReleaseTrust(releaseDir, platform) {
  if (platform !== "darwin" && platform !== "win32") {
    console.log(`release trust gate skipped for ${platform}`);
    return;
  }

  const subjects = await collectTrustSubjects(releaseDir, platform);
  for (const relativeSubject of subjects) {
    const absoluteSubject = path.join(releaseDir, relativeSubject);
    if (platform === "darwin") {
      await verifyMacTrustSubject(absoluteSubject);
      continue;
    }
    await verifyWindowsTrustSubject(absoluteSubject);
  }
}

async function verifyMacTrustSubject(subjectPath) {
  if (subjectPath.endsWith(".app")) {
    const codesignOutput = await runCommand("codesign", ["-dv", "--verbose=4", subjectPath]);
    assertMacCodesignOutput(codesignOutput, subjectPath);
  }
  const staplerOutput = await runCommand("xcrun", ["stapler", "validate", subjectPath]);
  assertMacStaplerOutput(staplerOutput, subjectPath);
}

async function verifyWindowsTrustSubject(subjectPath) {
  const status = await runCommand("powershell", [
    "-NoLogo",
    "-NonInteractive",
    "-Command",
    `(Get-AuthenticodeSignature -FilePath '${escapePowerShellLiteral(subjectPath)}').Status`,
  ]);
  assertAuthenticodeStatus(status, subjectPath);
}

async function walkReleaseTree(rootDir, currentDir, platform, subjects) {
  const entries = await readdir(currentDir, { withFileTypes: true });
  for (const entry of entries) {
    const absolutePath = path.join(currentDir, entry.name);
    const relativePath = toPosixRelative(rootDir, absolutePath);
    if (entry.isDirectory()) {
      if (platform === "darwin" && relativePath.endsWith(".app")) {
        subjects.push(relativePath);
        continue;
      }
      await walkReleaseTree(rootDir, absolutePath, platform, subjects);
      continue;
    }
    if (!entry.isFile()) {
      continue;
    }
    subjects.push(relativePath);
  }
}

function toPosixRelative(baseDir, targetPath) {
  return path.relative(baseDir, targetPath).split(path.sep).join("/");
}

function escapePowerShellLiteral(value) {
  return value.replace(/'/gu, "''");
}

async function runCommand(command, args) {
  return await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) {
        resolve(`${stdout}${stderr}`);
        return;
      }
      reject(
        new Error(
          `${command} ${args.join(" ")} exited with code ${code ?? 1}${stderr ? `: ${stderr.trim()}` : ""}`,
        ),
      );
    });
  });
}

async function main() {
  const argv = process.argv.slice(2);
  const platform = resolveTrustGatePlatform(argv, process.env);
  const releaseDir = resolveTrustGateReleaseDir(argv, process.env);
  await stat(releaseDir);
  await verifyReleaseTrust(releaseDir, platform);
  console.log(`release trust gate passed for ${platform}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  await main();
}
