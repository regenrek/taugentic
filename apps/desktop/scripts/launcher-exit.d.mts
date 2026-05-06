export interface LauncherExitStatus {
  exitCode?: number | undefined;
  signal?: string | undefined;
}

export function resolveLauncherExitCode(exitStatus?: LauncherExitStatus): number;
