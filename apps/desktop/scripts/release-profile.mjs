import { parseArgvFlagValue } from "./argv-flag.mjs";

const DESKTOP_RELEASE_PROFILES = ["stable", "nightly", "mission-control"];

const RELEASE_PROFILE_CONFIG = {
  stable: {
    appId: "app.taugentic.desktop",
    artifactStem: "taugentic-desktop",
    channel: "stable",
    packageName: "taugentic-desktop-app",
    productName: "Taugentic",
  },
  nightly: {
    appId: "app.taugentic.desktop.nightly",
    artifactStem: "taugentic-desktop-nightly",
    channel: "nightly",
    packageName: "taugentic-desktop-nightly-app",
    productName: "Taugentic Nightly",
  },
  "mission-control": {
    appId: "app.taugentic.desktop.mission-control",
    artifactStem: "taugentic-mission-control",
    channel: "mission-control",
    packageName: "taugentic-mission-control-app",
    productName: "Taugentic Mission Control",
  },
};

export function parseDesktopReleaseProfile(rawValue) {
  if (typeof rawValue !== "string") {
    return "stable";
  }
  const normalized = rawValue.trim().toLowerCase();
  if (normalized in RELEASE_PROFILE_CONFIG) {
    return normalized;
  }
  throw new Error(
    `unknown desktop release profile: ${rawValue}; expected one of ${DESKTOP_RELEASE_PROFILES.join(", ")}`,
  );
}

export function resolveDesktopReleaseProfile(argv = [], env = process.env) {
  const fromArgv = parseArgvFlagValue(argv, "--release-profile");
  const fromEnv = env.TAUGENTIC_DESKTOP_RELEASE_PROFILE;
  return parseDesktopReleaseProfile(fromArgv ?? fromEnv ?? "stable");
}

export function getDesktopReleaseProfileConfig(profile) {
  return RELEASE_PROFILE_CONFIG[parseDesktopReleaseProfile(profile)];
}

export function buildDesktopArtifactNameTemplate(profile) {
  const { artifactStem } = getDesktopReleaseProfileConfig(profile);
  return `${artifactStem}-\${version}-\${os}-\${arch}.\${ext}`;
}
