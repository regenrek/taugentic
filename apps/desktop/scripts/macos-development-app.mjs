import { constants } from "node:fs";
import { access, copyFile, mkdir, rm, writeFile } from "node:fs/promises";
import { delimiter, join, resolve } from "node:path";

export const developmentAppName = "Taugentic Development";
export const developmentBundleIdentifier = "dev.kregenrek.taugentic.desktop";

function plist(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function infoPlist() {
  return [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">',
    '<plist version="1.0">',
    "<dict>",
    "  <key>CFBundleDisplayName</key>",
    `  <string>${plist(developmentAppName)}</string>`,
    "  <key>CFBundleExecutable</key>",
    `  <string>${plist(developmentAppName)}</string>`,
    "  <key>CFBundleIdentifier</key>",
    `  <string>${developmentBundleIdentifier}</string>`,
    "  <key>CFBundleInfoDictionaryVersion</key>",
    "  <string>6.0</string>",
    "  <key>CFBundleName</key>",
    `  <string>${plist(developmentAppName)}</string>`,
    "  <key>CFBundlePackageType</key>",
    "  <string>APPL</string>",
    "  <key>CFBundleShortVersionString</key>",
    "  <string>0.0.0-development</string>",
    "  <key>CFBundleVersion</key>",
    "  <string>1</string>",
    "  <key>LSMinimumSystemVersion</key>",
    "  <string>13.0</string>",
    "  <key>NSMicrophoneUsageDescription</key>",
    "  <string>Taugentic uses the microphone only while you record a voice session.</string>",
    "  <key>NSScreenCaptureUsageDescription</key>",
    "  <string>Taugentic uses screen capture only when you explicitly request it.</string>",
    "</dict>",
    "</plist>",
    "",
  ].join("\n");
}

async function executableAt(path) {
  try {
    await access(path, constants.X_OK);
    return path;
  } catch {
    return undefined;
  }
}

export async function resolveBunExecutable(pathValue = process.env.PATH ?? "") {
  for (const directory of pathValue.split(delimiter)) {
    if (!directory) continue;
    const candidate = await executableAt(join(directory, "bun"));
    if (candidate) return candidate;
  }
  throw new Error("Bun executable was not found on PATH");
}

export async function materializeMacosDevelopmentApp({
  desktopRoot,
  executableSourcePath,
  entrypointPath,
}) {
  const resolvedDesktopRoot = resolve(desktopRoot);
  const source = await executableAt(executableSourcePath);
  if (!source)
    throw new Error(
      `Bundle executable is not executable: ${executableSourcePath}`,
    );

  const bundlePath = join(
    resolvedDesktopRoot,
    ".taugentic-development",
    `${developmentAppName}.app`,
  );
  const macosDirectory = join(bundlePath, "Contents", "MacOS");
  const executablePath = join(macosDirectory, developmentAppName);

  await rm(bundlePath, { recursive: true, force: true });
  await mkdir(macosDirectory, { recursive: true });
  await copyFile(source, executablePath, constants.COPYFILE_CLONE);
  await writeFile(
    join(bundlePath, "Contents", "Info.plist"),
    infoPlist(),
    "utf8",
  );
  await access(executablePath, constants.X_OK);

  return {
    bundlePath,
    executablePath,
    entrypointPath: entrypointPath ? resolve(entrypointPath) : undefined,
  };
}

export function macosDevelopmentAppLaunch({
  developmentApp,
  desktopRoot,
  daemonBinary,
  daemonSocketName,
  hot,
  applicationArguments = [],
  forwardStandardStreams = false,
}) {
  const runtimeArguments = developmentApp.entrypointPath
    ? [
        "--cwd",
        resolve(desktopRoot),
        ...(hot ? ["--hot"] : []),
        developmentApp.entrypointPath,
      ]
    : applicationArguments;
  return {
    command: "/usr/bin/open",
    arguments: [
      "-n",
      "-W",
      ...(forwardStandardStreams
        ? ["--stdout", "/dev/fd/1", "--stderr", "/dev/fd/2"]
        : []),
      ...(daemonBinary
        ? ["--env", `TAUGENTIC_DAEMON_BINARY=${daemonBinary}`]
        : []),
      ...(daemonSocketName
        ? ["--env", `TAUGENTIC_DAEMON_SOCKET_NAME=${daemonSocketName}`]
        : []),
      developmentApp.bundlePath,
      "--args",
      ...runtimeArguments,
    ],
  };
}
