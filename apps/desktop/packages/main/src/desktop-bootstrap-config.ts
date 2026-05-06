import { existsSync } from "node:fs";
import { posix, win32 } from "node:path";
import { fileURLToPath } from "node:url";

const DAEMON_BINARY_ENV_VAR = "TAUGENTIC_DAEMON_BINARY";
const ALLOW_PACKAGED_DAEMON_BINARY_OVERRIDE_ENV_VAR =
  "TAUGENTIC_ALLOW_PACKAGED_DAEMON_BINARY_OVERRIDE";

const DAEMON_BINARY_NAME = "ta-daemon";
const DAEMON_BOOTSTRAP_SUBCOMMAND = "__runtime-control-bootstrap";
const DESKTOP_DAEMON_ENV_PREFIXES = ["TAUGENTIC_DAEMON_", "TAUGENTIC_LOG_"];
const DESKTOP_DAEMON_ENV_KEYS = [
  "APPDATA",
  "CARGO_TARGET_DIR",
  "HOME",
  "PATH",
  "SYSTEMROOT",
  "TEMP",
  "TMP",
  "TMPDIR",
  "USERPROFILE",
  "XDG_CONFIG_HOME",
  "XDG_RUNTIME_DIR",
];

export interface DesktopDaemonBootstrapConfig {
  allowPackagedOverride: boolean;
  daemonBinaryOverride?: string;
  childEnv: NodeJS.ProcessEnv;
}

export interface DesktopDaemonBootstrapLaunchSpec {
  argsPrefix: string[];
  command: string;
  cwd?: string;
}

export interface ResolveDesktopDaemonBootstrapLaunchSpecOptions {
  commandName: string;
  cwd: string;
  env: NodeJS.ProcessEnv;
  isPackaged: boolean;
  platform: NodeJS.Platform;
  repoRoot: string;
  resourcesPath: string;
}

export function createDesktopDaemonBootstrapConfig(
  env: NodeJS.ProcessEnv = process.env,
): DesktopDaemonBootstrapConfig {
  const daemonBinaryOverride = env[DAEMON_BINARY_ENV_VAR]?.trim();

  return {
    allowPackagedOverride: Boolean(env[ALLOW_PACKAGED_DAEMON_BINARY_OVERRIDE_ENV_VAR]?.trim()),
    daemonBinaryOverride:
      daemonBinaryOverride && daemonBinaryOverride.length > 0 ? daemonBinaryOverride : undefined,
    childEnv: buildDesktopDaemonChildEnv(env),
  };
}

export function buildDesktopDaemonChildEnv(env: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const childEnv: NodeJS.ProcessEnv = {};

  for (const key of DESKTOP_DAEMON_ENV_KEYS) {
    const value = env[key];
    if (typeof value === "string" && value.length > 0) {
      childEnv[key] = value;
    }
  }

  for (const [key, value] of Object.entries(env)) {
    if (
      typeof value === "string" &&
      value.length > 0 &&
      DESKTOP_DAEMON_ENV_PREFIXES.some((prefix) => key.startsWith(prefix))
    ) {
      childEnv[key] = value;
    }
  }

  return childEnv;
}

export function resolveDesktopDaemonBootstrapLaunchSpec({
  commandName,
  cwd,
  env,
  isPackaged,
  platform,
  repoRoot,
  resourcesPath,
}: ResolveDesktopDaemonBootstrapLaunchSpecOptions): DesktopDaemonBootstrapLaunchSpec {
  const pathModule = platform === "win32" ? win32 : posix;
  const bootstrapConfig = createDesktopDaemonBootstrapConfig(env);
  const overrideBinary = bootstrapConfig.daemonBinaryOverride;
  if (overrideBinary && (!isPackaged || bootstrapConfig.allowPackagedOverride)) {
    const command = pathModule.isAbsolute(overrideBinary)
      ? overrideBinary
      : pathModule.resolve(cwd, overrideBinary);
    return {
      command,
      argsPrefix: [DAEMON_BOOTSTRAP_SUBCOMMAND, commandName],
      cwd: pathModule.dirname(command),
    };
  }

  if (isPackaged) {
    const command = resolvePackagedDaemonBinaryPath(platform, resourcesPath);
    return {
      command,
      argsPrefix: [DAEMON_BOOTSTRAP_SUBCOMMAND, commandName],
    };
  }

  return {
    command: "cargo",
    argsPrefix: [
      "run",
      "--package",
      "ta-orchestrator",
      "--bin",
      DAEMON_BINARY_NAME,
      "--",
      DAEMON_BOOTSTRAP_SUBCOMMAND,
      commandName,
    ],
    cwd: repoRoot,
  };
}

export function resolvePackagedDaemonBinaryPath(
  platform: NodeJS.Platform,
  resourcesPath: string,
): string {
  const fileName = platform === "win32" ? `${DAEMON_BINARY_NAME}.exe` : DAEMON_BINARY_NAME;
  const pathModule = platform === "win32" ? win32 : posix;
  return pathModule.join(resourcesPath, "bin", fileName);
}

export function resolveDesktopRepoRoot(importMetaUrl: string): string {
  return fileURLToPath(new URL("../../../../../", importMetaUrl));
}

export function assertDesktopBootstrapLaunchSpecExists(
  launchSpec: DesktopDaemonBootstrapLaunchSpec,
  isPackaged: boolean,
): void {
  if (!isPackaged || existsSync(launchSpec.command)) {
    return;
  }

  throw new Error(
    `bundled ${DAEMON_BINARY_NAME} executable is missing at ${launchSpec.command}; package the daemon binary or set ${DAEMON_BINARY_ENV_VAR} with ${ALLOW_PACKAGED_DAEMON_BINARY_OVERRIDE_ENV_VAR}`,
  );
}
