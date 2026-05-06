type ReleaseScriptEnv = Record<string, string | undefined>;

export type DesktopReleasePublishMode = "always" | "never";

export function isDesktopReleaseTagRef(ref: string | null | undefined): boolean;
export function extractDesktopReleaseTag(ref: string | null | undefined): string | null;
export function resolveDesktopReleaseRef(argv?: string[], env?: ReleaseScriptEnv): string | null;
export function resolveDesktopReleasePublishModeForRef(
  ref: string | null | undefined,
  packageVersion: string,
  releaseProfile: "stable" | "nightly" | "mission-control",
): DesktopReleasePublishMode;
export function resolveDesktopReleasePublishMode(
  argv?: string[],
  env?: ReleaseScriptEnv,
  rootDir?: string,
): Promise<DesktopReleasePublishMode>;
