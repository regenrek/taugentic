type ReleaseScriptEnv = Record<string, string | undefined>;

export function resolveTrustGatePlatform(argv?: string[], env?: ReleaseScriptEnv): string;
export function resolveTrustGateReleaseDir(argv?: string[], env?: ReleaseScriptEnv): string;
export function isMacTrustSubject(relativePath: string): boolean;
export function isWindowsTrustSubject(relativePath: string): boolean;
export function assertMacCodesignOutput(output: string, subjectPath: string): void;
export function assertMacStaplerOutput(output: string, subjectPath: string): void;
export function assertAuthenticodeStatus(output: string, subjectPath: string): void;
export function collectTrustSubjects(releaseDir: string, platform: string): Promise<string[]>;
export function verifyReleaseTrust(releaseDir: string, platform: string): Promise<void>;
