type ReleaseAggregateEnv = Record<string, string | undefined>;

export function resolveReleaseAggregateInputDir(argv?: string[], env?: ReleaseAggregateEnv): string;

export function normalizeAggregatedReleaseRelativePath(relativePath: string): string;

export function aggregateDownloadedReleaseArtifacts(options: {
  inputDir: string;
  outputDir: string;
}): Promise<string[]>;
