type DesktopPlatform = string;
type ProcessEnvShape = Record<string, string | undefined>;

export const desktopRootDir: string;
export const repoRootDir: string;
export const artifactRootDir: string;
export const stagedAppDir: string;
export const stagedResourcesDir: string;
export const stagedResourcesBinDir: string;
export const stagedAppMainEntry: string;

export function parseArgvFlagValue(argv: string[], flagName: string): string | null;
export function normalizeStageTargetPlatform(
  raw: string,
  fallbackPlatform: DesktopPlatform,
): DesktopPlatform;
export function resolveStageTargetPlatform(
  argv: string[],
  env: ProcessEnvShape,
  fallbackPlatform: DesktopPlatform,
): DesktopPlatform;
export function resolveStageCargoTargetTriple(argv: string[], env: ProcessEnvShape): string | null;
export function assertStageTargetPlatformConfiguration(
  targetPlatform: DesktopPlatform,
  hostPlatform: DesktopPlatform,
  cargoTargetTriple: string | null,
): void;
export function daemonBinaryFileNameForPlatform(platform: DesktopPlatform): string;
export function daemonControlBinaryFileNameForPlatform(platform: DesktopPlatform): string;
export function buildStageAppPackageManifest(options: {
  releaseProfile?: "stable" | "nightly" | "mission-control";
  version: string;
}): {
  name: string;
  author: string;
  description: string;
  private: true;
  type: "module";
  version: string;
  main: string;
  dependencies: {
    "@taugentic/desktop-shared": string;
  };
};
export function getDesktopReleaseProfileConfig(profile: string): {
  appId: string;
  artifactStem: string;
  channel: string;
  packageName: string;
  productName: string;
};
export function resolveDesktopReleaseProfile(
  argv: string[],
  env: ProcessEnvShape,
): "stable" | "nightly" | "mission-control";
export function stageCopyPlan(targetAppDir?: string): Array<{ from: string; to: string }>;
