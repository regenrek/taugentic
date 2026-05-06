import { describe, expect, it } from "vite-plus/test";

import {
  resolveDaemonSocketName,
  resolveDaemonSocketPathForPlatform,
} from "../../packages/shared/src/daemon.js";

describe("resolveDaemonSocketName", () => {
  it("falls back to the default socket name for an empty override", () => {
    expect(resolveDaemonSocketName("ta-daemon", "")).toBe("ta-daemon");
  });

  it("falls back to the default socket name for a whitespace override", () => {
    expect(resolveDaemonSocketName("ta-daemon", "   ")).toBe("ta-daemon");
  });

  it("trims non-empty override values", () => {
    expect(resolveDaemonSocketName("ta-daemon", "  ta-daemon-smoke  ")).toBe("ta-daemon-smoke");
  });
});

describe("resolveDaemonSocketPathForPlatform", () => {
  it("uses the runtime dir when one is provided", () => {
    expect(
      resolveDaemonSocketPathForPlatform(
        "linux",
        "ta-daemon",
        "/run/user/501",
        "/tmp",
        501,
        "/Users/alice",
      ),
    ).toBe("/run/user/501/ta-daemon.sock");
  });

  it("uses a user-scoped temp fallback on linux when runtime dir is missing", () => {
    expect(
      resolveDaemonSocketPathForPlatform(
        "linux",
        "ta-daemon",
        undefined,
        "/tmp",
        501,
        "/Users/alice",
      ),
    ).toBe("/tmp/taugentic-uid-501/ta-daemon.sock");
  });

  it("treats whitespace runtime dir as missing on macos", () => {
    expect(
      resolveDaemonSocketPathForPlatform(
        "darwin",
        "ta-daemon",
        "   ",
        "/var/folders/zz/temporary",
        501,
        "/Users/alice",
      ),
    ).toBe("/Users/alice/Library/Application Support/taugentic/runtime/ta-daemon.sock");
  });

  it("treats whitespace home as missing on macos", () => {
    expect(
      resolveDaemonSocketPathForPlatform(
        "darwin",
        "ta-daemon",
        undefined,
        "/var/folders/zz/temporary",
        501,
        "   ",
      ),
    ).toBe("/tmp/taugentic/runtime/ta-daemon.sock");
  });

  it("uses a stable home-scoped runtime dir on macos when runtime dir is missing", () => {
    expect(
      resolveDaemonSocketPathForPlatform(
        "darwin",
        "ta-daemon",
        undefined,
        "/var/folders/zz/temporary",
        501,
        "/Users/alice",
      ),
    ).toBe("/Users/alice/Library/Application Support/taugentic/runtime/ta-daemon.sock");
  });

  it("falls back to a fixed tmp runtime dir on macos when home is unavailable", () => {
    expect(
      resolveDaemonSocketPathForPlatform(
        "darwin",
        "ta-daemon",
        undefined,
        "/var/folders/zz/temporary",
        501,
        undefined,
      ),
    ).toBe("/tmp/taugentic/runtime/ta-daemon.sock");
  });

  it("uses a short stable macos fallback when the primary socket path would be too long", () => {
    const longHome = `/Users/${"a".repeat(80)}`;

    expect(
      resolveDaemonSocketPathForPlatform(
        "darwin",
        "ta-daemon",
        undefined,
        "/var/folders/zz/temporary",
        501,
        longHome,
      ),
    ).toBe("/tmp/taugentic/s/ta-22fb689c979da975.sock");
  });

  it("uses a named pipe on windows", () => {
    expect(
      resolveDaemonSocketPathForPlatform(
        "win32",
        "ta-daemon",
        undefined,
        "C:\\Temp",
        undefined,
        undefined,
      ),
    ).toBe("\\\\.\\pipe\\ta-daemon");
  });
});
