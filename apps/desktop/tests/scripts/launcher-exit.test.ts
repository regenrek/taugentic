import { describe, expect, it } from "vite-plus/test";

import { resolveLauncherExitCode } from "../../scripts/launcher-exit.mjs";

describe("resolveLauncherExitCode", () => {
  it("preserves explicit exit codes", () => {
    expect(resolveLauncherExitCode({ exitCode: 17 })).toBe(17);
  });

  it("maps signals to shell-visible exit codes", () => {
    expect(resolveLauncherExitCode({ signal: "SIGTERM" })).toBe(143);
    expect(resolveLauncherExitCode({ signal: "SIGINT" })).toBe(130);
  });

  it("falls back to 1 for unknown signal names", () => {
    expect(resolveLauncherExitCode({ signal: "SIGUNKNOWN" })).toBe(1);
  });

  it("defaults to 0 for a clean shutdown without failure details", () => {
    expect(resolveLauncherExitCode()).toBe(0);
  });
});
