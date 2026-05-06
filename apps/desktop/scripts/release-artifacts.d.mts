type ReleaseScriptEnv = Record<string, string | undefined>;

export interface ReleaseArtifactManifestEntry {
  path: string;
  sha256: string;
  sizeBytes: number;
  workspacePath: string;
}

export interface WriteReleaseArtifactsBundleOptions {
  releaseDir: string;
  manifestPath?: string | undefined;
  checksumPath?: string | undefined;
  subjectsPath?: string | undefined;
}

export interface WriteReleaseArtifactsBundleResult {
  checksumPath: string;
  manifest: ReleaseArtifactManifestEntry[];
  manifestPath: string;
  subjectsPath: string;
}

export function resolveReleaseDir(argv?: string[], env?: ReleaseScriptEnv): string;
export function isGeneratedReleaseMetadataFile(relativePath: string): boolean;
export function isRecognizedReleaseArtifact(relativePath: string): boolean;
export function isPrimaryReleaseArtifact(relativePath: string): boolean;
export function collectReleaseArtifactPaths(releaseDir: string): Promise<string[]>;
export function buildReleaseArtifactManifest(
  releaseDir: string,
): Promise<ReleaseArtifactManifestEntry[]>;
export function formatReleaseChecksums(manifest: readonly ReleaseArtifactManifestEntry[]): string;
export function formatAttestationSubjects(
  manifest: readonly ReleaseArtifactManifestEntry[],
): string;
export function writeReleaseArtifactsBundle(
  options: WriteReleaseArtifactsBundleOptions,
): Promise<WriteReleaseArtifactsBundleResult>;
