import { mkdtemp, mkdir, readFile, readdir, rename, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { tmpdir } from "node:os";

import { afterEach, describe, expect, it } from "vite-plus/test";

import { stageDesktopPackage } from "../../scripts/stage-desktop-package.mjs";

const tempDirs: string[] = [];

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map((dir) => rm(dir, { force: true, recursive: true })));
});

describe("stageDesktopPackage", () => {
  it("keeps the previous stage when stage install fails", async () => {
    const fixture = await createStageFixture();
    await seedCurrentStage(fixture.stageRootDir, "old-app", "old-daemon");
    await writeStageSource(fixture.sourceRootDir, "new-app");

    await expect(
      stageDesktopPackage({
        cargoTargetTriple: null,
        copyPlanBuilder: fixture.copyPlanBuilder,
        mainPackageVersion: "1.2.3",
        releaseProfile: "stable",
        resolveCargoTargetDir: async () => fixture.cargoTargetDir,
        runCommand: async (name: string) => {
          if (name === "stage:install") {
            throw new Error("stage:install exited with code 1");
          }
        },
        skipDaemonBuild: true,
        skipDesktopBuild: true,
        skipInstall: false,
        stageRootDir: fixture.stageRootDir,
        targetPlatform: "darwin",
      }),
    ).rejects.toThrow("stage:install exited with code 1");

    await expect(
      readFile(path.join(fixture.stageRootDir, "package-app", "current.txt"), "utf8"),
    ).resolves.toBe("old-app");
    await expect(
      readFile(path.join(fixture.stageRootDir, "resources", "bin", "ta-daemon"), "utf8"),
    ).resolves.toBe("old-daemon");
    await expect(listVisibleStageEntries(fixture.stageRootDir)).resolves.toEqual([
      "package-app",
      "resources",
    ]);
  });

  it("keeps the previous stage when daemon staging fails after install", async () => {
    const fixture = await createStageFixture();
    await seedCurrentStage(fixture.stageRootDir, "old-app", "old-daemon");
    await writeStageSource(fixture.sourceRootDir, "new-app");

    await expect(
      stageDesktopPackage({
        cargoTargetTriple: null,
        copyPlanBuilder: fixture.copyPlanBuilder,
        mainPackageVersion: "1.2.3",
        releaseProfile: "stable",
        resolveCargoTargetDir: async () => fixture.cargoTargetDir,
        runCommand: async () => {},
        skipDaemonBuild: true,
        skipDesktopBuild: true,
        skipInstall: false,
        stageRootDir: fixture.stageRootDir,
        targetPlatform: "darwin",
      }),
    ).rejects.toThrow(
      `required staging input is missing: ${path.join(fixture.cargoTargetDir, "release", "ta-daemon")}`,
    );

    await expect(
      readFile(path.join(fixture.stageRootDir, "package-app", "current.txt"), "utf8"),
    ).resolves.toBe("old-app");
    await expect(
      readFile(path.join(fixture.stageRootDir, "resources", "bin", "ta-daemon"), "utf8"),
    ).resolves.toBe("old-daemon");
    await expect(listVisibleStageEntries(fixture.stageRootDir)).resolves.toEqual([
      "package-app",
      "resources",
    ]);
  });

  it("promotes a fully prepared pending stage only after success", async () => {
    const fixture = await createStageFixture();
    await seedCurrentStage(fixture.stageRootDir, "old-app", "old-daemon");
    await writeStageSource(fixture.sourceRootDir, "new-app");
    await mkdir(path.join(fixture.cargoTargetDir, "release"), { recursive: true });
    await writeFile(
      path.join(fixture.cargoTargetDir, "release", "ta-daemon"),
      "new-daemon",
      "utf8",
    );

    await stageDesktopPackage({
      cargoTargetTriple: null,
      copyPlanBuilder: fixture.copyPlanBuilder,
      mainPackageVersion: "1.2.3",
      releaseProfile: "stable",
      resolveCargoTargetDir: async () => fixture.cargoTargetDir,
      runCommand: async () => {},
      skipDaemonBuild: true,
      skipDesktopBuild: true,
      skipInstall: false,
      stageRootDir: fixture.stageRootDir,
      targetPlatform: "darwin",
    });

    await expect(
      readFile(
        path.join(fixture.stageRootDir, "package-app", "packages", "main", "dist", "index.js"),
        "utf8",
      ),
    ).resolves.toBe("new-app");
    await expect(
      stat(path.join(fixture.stageRootDir, "package-app", "current.txt")),
    ).rejects.toThrow();
    await expect(
      readFile(path.join(fixture.stageRootDir, "package-app", "package.json"), "utf8").then(
        JSON.parse,
      ),
    ).resolves.toMatchObject({
      main: "packages/main/dist/index.js",
      name: "taugentic-desktop-app",
      version: "1.2.3",
    });
    await expect(
      readFile(path.join(fixture.stageRootDir, "resources", "bin", "ta-daemon"), "utf8"),
    ).resolves.toBe("new-daemon");
    await expect(listVisibleStageEntries(fixture.stageRootDir)).resolves.toEqual([
      "package-app",
      "resources",
    ]);
  });

  it("replaces the staged app manifest with the selected release-profile identity", async () => {
    const fixture = await createStageFixture();
    await seedCurrentStage(fixture.stageRootDir, "old-app", "old-daemon");
    await writeStageSource(fixture.sourceRootDir, "new-app");
    await mkdir(path.join(fixture.cargoTargetDir, "release"), { recursive: true });
    await writeFile(
      path.join(fixture.cargoTargetDir, "release", "ta-daemon"),
      "new-daemon",
      "utf8",
    );

    await stageDesktopPackage({
      cargoTargetTriple: null,
      copyPlanBuilder: fixture.copyPlanBuilder,
      mainPackageVersion: "2.3.4",
      releaseProfile: "mission-control",
      resolveCargoTargetDir: async () => fixture.cargoTargetDir,
      runCommand: async () => {},
      skipDaemonBuild: true,
      skipDesktopBuild: true,
      skipInstall: false,
      stageRootDir: fixture.stageRootDir,
      targetPlatform: "darwin",
    });

    await expect(
      readFile(path.join(fixture.stageRootDir, "package-app", "package.json"), "utf8").then(
        JSON.parse,
      ),
    ).resolves.toMatchObject({
      description: "Taugentic Mission Control desktop runtime shell",
      name: "taugentic-mission-control-app",
      version: "2.3.4",
    });
    await expect(
      stat(path.join(fixture.stageRootDir, "package-app", "current.txt")),
    ).rejects.toThrow();
  });

  it("restores the previous stage when package-app promote fails after resources already moved", async () => {
    const fixture = await createStageFixture();
    await seedCurrentStage(fixture.stageRootDir, "old-app", "old-daemon");
    await writeStageSource(fixture.sourceRootDir, "new-app");
    await mkdir(path.join(fixture.cargoTargetDir, "release"), { recursive: true });
    await writeFile(
      path.join(fixture.cargoTargetDir, "release", "ta-daemon"),
      "new-daemon",
      "utf8",
    );

    await expect(
      stageDesktopPackage({
        cargoTargetTriple: null,
        copyPlanBuilder: fixture.copyPlanBuilder,
        mainPackageVersion: "1.2.3",
        releaseProfile: "stable",
        renamePath: async (from: string, to: string) => {
          if (
            path.basename(from) === "package-app" &&
            path.basename(to) === "package-app" &&
            from.includes(".pending-stage-")
          ) {
            throw new Error("simulated promote failure");
          }
          await rename(from, to);
        },
        resolveCargoTargetDir: async () => fixture.cargoTargetDir,
        runCommand: async () => {},
        skipDaemonBuild: true,
        skipDesktopBuild: true,
        skipInstall: false,
        stageRootDir: fixture.stageRootDir,
        targetPlatform: "darwin",
      }),
    ).rejects.toThrow("simulated promote failure");

    await expect(
      readFile(path.join(fixture.stageRootDir, "package-app", "current.txt"), "utf8"),
    ).resolves.toBe("old-app");
    await expect(
      readFile(path.join(fixture.stageRootDir, "resources", "bin", "ta-daemon"), "utf8"),
    ).resolves.toBe("old-daemon");
    await expect(listVisibleStageEntries(fixture.stageRootDir)).resolves.toEqual([
      "package-app",
      "resources",
    ]);
  });

  it("keeps the backup stage recoverable when rollback restore fails after promote error", async () => {
    const fixture = await createStageFixture();
    await seedCurrentStage(fixture.stageRootDir, "old-app", "old-daemon");
    await writeStageSource(fixture.sourceRootDir, "new-app");
    await mkdir(path.join(fixture.cargoTargetDir, "release"), { recursive: true });
    await writeFile(
      path.join(fixture.cargoTargetDir, "release", "ta-daemon"),
      "new-daemon",
      "utf8",
    );

    await expect(
      stageDesktopPackage({
        cargoTargetTriple: null,
        copyPlanBuilder: fixture.copyPlanBuilder,
        mainPackageVersion: "1.2.3",
        releaseProfile: "stable",
        renamePath: async (from: string, to: string) => {
          if (
            path.basename(from) === "package-app" &&
            path.basename(to) === "package-app" &&
            from.includes(".pending-stage-")
          ) {
            throw new Error("simulated promote failure");
          }
          if (
            path.basename(from) === "package-app" &&
            path.basename(to) === "package-app" &&
            from.includes(".backup-stage-")
          ) {
            throw new Error("simulated restore failure");
          }
          await rename(from, to);
        },
        resolveCargoTargetDir: async () => fixture.cargoTargetDir,
        runCommand: async () => {},
        skipDaemonBuild: true,
        skipDesktopBuild: true,
        skipInstall: false,
        stageRootDir: fixture.stageRootDir,
        targetPlatform: "darwin",
      }),
    ).rejects.toThrow(
      /failed to restore previous staged package after promote error; backup kept at /u,
    );

    await expect(stat(path.join(fixture.stageRootDir, "package-app"))).rejects.toThrow();
    await expect(
      readFile(path.join(fixture.stageRootDir, "resources", "bin", "ta-daemon"), "utf8"),
    ).resolves.toBe("old-daemon");

    const backupRoots = await listHiddenStageEntries(fixture.stageRootDir, ".backup-stage-");
    expect(backupRoots).toHaveLength(1);
    await expect(
      readFile(
        path.join(fixture.stageRootDir, backupRoots[0], "package-app", "current.txt"),
        "utf8",
      ),
    ).resolves.toBe("old-app");

    const pendingRoots = await listHiddenStageEntries(fixture.stageRootDir, ".pending-stage-");
    expect(pendingRoots).toHaveLength(1);
    await expect(
      readFile(
        path.join(
          fixture.stageRootDir,
          pendingRoots[0],
          "package-app",
          "packages",
          "main",
          "dist",
          "index.js",
        ),
        "utf8",
      ),
    ).resolves.toBe("new-app");
  });
});

async function createStageFixture() {
  const rootDir = await mkdtemp(path.join(tmpdir(), "taugentic-stage-package-"));
  tempDirs.push(rootDir);
  const stageRootDir = path.join(rootDir, ".artifacts");
  const sourceRootDir = path.join(rootDir, "source-main-dist");
  const cargoTargetDir = path.join(rootDir, "cargo-target");
  return {
    cargoTargetDir,
    copyPlanBuilder: (targetAppDir: string) => [
      {
        from: sourceRootDir,
        to: path.join(targetAppDir, "packages", "main", "dist"),
      },
    ],
    sourceRootDir,
    stageRootDir,
  };
}

async function seedCurrentStage(stageRootDir: string, appContents: string, daemonContents: string) {
  await mkdir(path.join(stageRootDir, "package-app"), { recursive: true });
  await mkdir(path.join(stageRootDir, "resources", "bin"), { recursive: true });
  await writeFile(path.join(stageRootDir, "package-app", "current.txt"), appContents, "utf8");
  await writeFile(path.join(stageRootDir, "resources", "bin", "ta-daemon"), daemonContents, "utf8");
}

async function writeStageSource(sourceRootDir: string, contents: string) {
  await mkdir(sourceRootDir, { recursive: true });
  await writeFile(path.join(sourceRootDir, "index.js"), contents, "utf8");
}

async function listVisibleStageEntries(stageRootDir: string) {
  return (await readdir(stageRootDir)).filter((entry) => !entry.startsWith(".")).sort();
}

async function listHiddenStageEntries(stageRootDir: string, prefix: string) {
  return (await readdir(stageRootDir)).filter((entry) => entry.startsWith(prefix)).sort();
}
