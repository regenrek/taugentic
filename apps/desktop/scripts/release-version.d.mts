export function normalizeReleaseTagVersion(tagName: string | null | undefined): string | null;
export function readDesktopMainPackageVersion(rootDir?: string): Promise<string>;
export function assertReleaseTagMatchesPackageVersion(
  tagName: string | null | undefined,
  packageVersion: string,
): void;
