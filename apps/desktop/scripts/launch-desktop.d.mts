export function isLaunchDesktopEntrypoint(argv?: string[], importMetaUrl?: string): boolean;
export interface DesktopDevPreflightStep {
  command: string[];
  cwd: string;
  name: string;
}
export interface DesktopDaemonCleanupStepOptions {
  includeProductSocket: boolean;
  name: string;
}
export function parseDesktopDevOrphanProcessIds(processTable: string, scopeDir?: string): number[];
export function resolveDaemonBootstrapCommand(action: string): string[];
export function resolveDesktopDaemonCleanupStep(
  rootDir?: string,
  options?: DesktopDaemonCleanupStepOptions,
): DesktopDevPreflightStep;
export function resolveDesktopDevPreflightCommands(rootDir?: string): DesktopDevPreflightStep[];
export function resolveElectronLaunchArguments(
  env?: Partial<Record<string, string | undefined>>,
): string[];
export function shouldForceCleanupForDaemonStatus(status: { actualMode?: string }): boolean;
