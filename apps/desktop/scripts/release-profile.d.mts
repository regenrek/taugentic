type ReleaseScriptEnv = Record<string, string | undefined>;

export type DesktopReleaseProfile = "stable" | "nightly" | "mission-control";

export interface DesktopReleaseProfileConfig {
  appId: string;
  artifactStem: string;
  channel: DesktopReleaseProfile;
  packageName: string;
  productName: string;
}

export function parseDesktopReleaseProfile(
  rawValue: string | null | undefined,
): DesktopReleaseProfile;
export function resolveDesktopReleaseProfile(
  argv?: string[],
  env?: ReleaseScriptEnv,
): DesktopReleaseProfile;
export function getDesktopReleaseProfileConfig(
  profile: string | null | undefined,
): DesktopReleaseProfileConfig;
export function buildDesktopArtifactNameTemplate(profile: string | null | undefined): string;
