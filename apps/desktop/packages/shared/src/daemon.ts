export function resolveDaemonSocketName(defaultAppName: string, overrideName?: string): string {
  const trimmedOverrideName = overrideName?.trim();
  return trimmedOverrideName ? trimmedOverrideName : defaultAppName;
}

const MACOS_RUNTIME_DIR_FALLBACK = "/tmp/taugentic/runtime";
const MACOS_SHORT_SOCKET_DIR = "/tmp/taugentic/s";
const MACOS_MAX_SOCKET_PATH_UTF8_BYTES = 103;
const FNV64_OFFSET_BASIS = 0xcbf29ce484222325n;
const FNV64_PRIME = 0x100000001b3n;
const FNV64_MASK = 0xffffffffffffffffn;
const UTF8_ENCODER = new TextEncoder();

function joinUnixPath(...segments: string[]): string {
  return segments
    .map((segment, index) =>
      index === 0 ? segment.replace(/\/+$/u, "") : segment.replace(/^\/+|\/+$/gu, ""),
    )
    .filter(Boolean)
    .join("/");
}

function normalizeEnvPath(value: string | undefined): string | undefined {
  const trimmedValue = value?.trim();
  return trimmedValue ? trimmedValue : undefined;
}

function utf8ByteLength(value: string): number {
  return UTF8_ENCODER.encode(value).length;
}

function fnv1a64Hex(value: string): string {
  let hash = FNV64_OFFSET_BASIS;
  for (const byte of UTF8_ENCODER.encode(value)) {
    hash ^= BigInt(byte);
    hash = (hash * FNV64_PRIME) & FNV64_MASK;
  }

  return hash.toString(16).padStart(16, "0");
}

function resolveStableMacosRuntimeDir(homeDir: string | undefined): string {
  const normalizedHomeDir = normalizeEnvPath(homeDir);
  return normalizedHomeDir
    ? joinUnixPath(normalizedHomeDir, "Library", "Application Support", "taugentic", "runtime")
    : MACOS_RUNTIME_DIR_FALLBACK;
}

function resolveStableMacosShortSocketPath(appName: string, homeDir: string | undefined): string {
  const normalizedHomeDir = normalizeEnvPath(homeDir) ?? "";
  const digest = fnv1a64Hex(`${normalizedHomeDir}\u0000${appName}`);
  return joinUnixPath(MACOS_SHORT_SOCKET_DIR, `ta-${digest}.sock`);
}

function applyMacosSocketPathGuard(
  socketPath: string,
  appName: string,
  homeDir: string | undefined,
): string {
  if (utf8ByteLength(socketPath) <= MACOS_MAX_SOCKET_PATH_UTF8_BYTES) {
    return socketPath;
  }

  return resolveStableMacosShortSocketPath(appName, homeDir);
}

export function resolveDaemonSocketPathForPlatform(
  platform: NodeJS.Platform,
  appName: string,
  runtimeDir: string | undefined,
  tempDir: string,
  userId: number | undefined,
  homeDir: string | undefined,
): string {
  if (platform === "win32") {
    return `\\\\.\\pipe\\${appName}`;
  }

  const normalizedRuntimeDir = normalizeEnvPath(runtimeDir);
  const socketFileName = `${appName}.sock`;
  if (normalizedRuntimeDir) {
    const socketPath = joinUnixPath(normalizedRuntimeDir, socketFileName);
    return platform === "darwin"
      ? applyMacosSocketPathGuard(socketPath, appName, homeDir)
      : socketPath;
  }

  if (platform === "darwin") {
    return applyMacosSocketPathGuard(
      joinUnixPath(resolveStableMacosRuntimeDir(homeDir), socketFileName),
      appName,
      homeDir,
    );
  }

  const baseDir =
    platform === "linux" && userId !== undefined
      ? joinUnixPath(tempDir, `taugentic-uid-${userId}`)
      : tempDir;
  return joinUnixPath(baseDir, socketFileName);
}
