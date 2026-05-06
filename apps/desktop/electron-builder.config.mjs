import path from "node:path";
import { readFileSync } from "node:fs";

import {
  desktopRootDir,
  getDesktopReleaseProfileConfig,
  resolveDesktopReleaseProfile,
  stagedAppDir,
  stagedResourcesBinDir,
} from "./scripts/package-layout.mjs";
import { buildDesktopArtifactNameTemplate } from "./scripts/release-profile.mjs";

const relativeFromDesktopRoot = (targetPath) => path.relative(desktopRootDir, targetPath);
const releaseProfile = resolveDesktopReleaseProfile(process.argv.slice(2), process.env);
const releaseProfileConfig = getDesktopReleaseProfileConfig(releaseProfile);
const mainPackageManifest = JSON.parse(
  readFileSync(path.join(desktopRootDir, "packages", "main", "package.json"), "utf8"),
);
const electronVersion = String(mainPackageManifest.devDependencies?.electron ?? "").replace(
  /^[~^]/u,
  "",
);
export default {
  appId: releaseProfileConfig.appId,
  artifactName: buildDesktopArtifactNameTemplate(releaseProfile),
  electronVersion,
  productName: releaseProfileConfig.productName,
  directories: {
    app: relativeFromDesktopRoot(stagedAppDir),
    output: "release",
  },
  files: ["package.json", "node_modules/**/*", "packages/**/*"],
  extraResources: [
    {
      from: relativeFromDesktopRoot(stagedResourcesBinDir),
      to: "bin",
      filter: ["ta-daemon", "ta-daemon.exe"],
    },
  ],
  mac: {
    category: "public.app-category.developer-tools",
    forceCodeSigning: true,
    target: ["dmg"],
  },
  linux: {
    category: "Development",
    target: ["deb"],
  },
  nsis: {
    allowToChangeInstallationDirectory: true,
    oneClick: false,
  },
  win: {
    forceCodeSigning: true,
    target: ["nsis"],
  },
};
