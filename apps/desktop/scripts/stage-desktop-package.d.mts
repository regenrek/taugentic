export type StageCommandRunner = (
  name: string,
  command: string[],
  cwd: string,
) => Promise<string | void>;

export type StageRenamePath = (from: string, to: string) => Promise<void>;

export type StageCopyPlanBuilder = (targetAppDir: string) => Array<{
  from: string;
  to: string;
}>;

export type StageDesktopPackageOptions = {
  cargoTargetTriple: string | null;
  copyPlanBuilder?: StageCopyPlanBuilder;
  mainPackageVersion?: string;
  renamePath?: StageRenamePath;
  releaseProfile: "stable" | "nightly" | "mission-control";
  resolveCargoTargetDir?: () => Promise<string>;
  runCommand?: StageCommandRunner;
  skipDaemonBuild: boolean;
  skipDesktopBuild: boolean;
  skipInstall: boolean;
  stageRootDir?: string;
  targetPlatform: string;
};

export function resolveStageDesktopPackageOptions(
  argv?: string[],
  env?: Record<string, string | undefined>,
  processPlatform?: string,
): StageDesktopPackageOptions;

export function stageDesktopPackage(options: StageDesktopPackageOptions): Promise<void>;
