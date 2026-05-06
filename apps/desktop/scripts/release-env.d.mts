type ReleaseScriptEnv = Record<string, string | undefined>;
type ReleaseTargetPlatform = "darwin" | "linux" | "win32";

export function requiredReleaseSigningEnv(platform: ReleaseTargetPlatform): readonly string[];
export function missingReleaseSigningEnv(
  platform: ReleaseTargetPlatform,
  env: ReleaseScriptEnv,
): string[];
export function assertReleaseSigningEnv(
  platform: ReleaseTargetPlatform,
  env: ReleaseScriptEnv,
): void;
