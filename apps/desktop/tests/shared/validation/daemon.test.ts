import { describe, expect, it } from "vite-plus/test";

import {
  parseDaemonControlStatusResult,
  parseDaemonDiagnostics,
  parseDaemonInitializeResult,
  parseDaemonStatusResult,
  parseDaemonSubscribeResult,
  ProtocolValidationError,
} from "../../../packages/shared/src/validation.js";
import { daemonCursor } from "./helpers.js";

describe("parseDaemonStatusResult", () => {
  it("accepts Rust-owned daemon status payloads", () => {
    expect(
      parseDaemonStatusResult({
        ready: true,
        daemonInstanceId: "daemon-1",
        runtimeMode: "local",
        socketPath: "/tmp/ta-daemon.sock",
        logPath: "/tmp/taugentic-daemon/ta-daemon/ta-daemon.log.jsonl",
        version: "0.0.1",
      }),
    ).toEqual({
      ready: true,
      daemonInstanceId: "daemon-1",
      runtimeMode: "local",
      socketPath: "/tmp/ta-daemon.sock",
      logPath: "/tmp/taugentic-daemon/ta-daemon/ta-daemon.log.jsonl",
      version: "0.0.1",
    });
  });

  it("rejects payloads that drift from the Rust schema", () => {
    expect(() =>
      parseDaemonStatusResult({
        ready: "yes",
        daemonInstanceId: "daemon-1",
        runtimeMode: "local",
        socketPath: "/tmp/ta-daemon.sock",
        logPath: "/tmp/taugentic-daemon/ta-daemon/ta-daemon.log.jsonl",
        version: "0.0.1",
      }),
    ).toThrow(ProtocolValidationError);
  });
});

describe("parseDaemonControlStatusResult", () => {
  it("accepts Rust-owned control status payloads", () => {
    expect(
      parseDaemonControlStatusResult({
        backgroundOptIn: true,
        desiredMode: "background",
        actualMode: "background",
        transitionStatus: "idle",
        reconcileRequired: false,
        allowedActions: ["stop", "disableBackground"],
        message: "Background mode is the desired runtime.",
        socketPath: "/tmp/ta-daemon.sock",
        logPath: "/tmp/taugentic-daemon/ta-daemon/ta-daemon.log.jsonl",
        daemonVersion: "0.0.1",
        protocolVersion: "2026-04-stage2",
      }),
    ).toEqual({
      backgroundOptIn: true,
      desiredMode: "background",
      actualMode: "background",
      transitionStatus: "idle",
      reconcileRequired: false,
      allowedActions: ["stop", "disableBackground"],
      message: "Background mode is the desired runtime.",
      socketPath: "/tmp/ta-daemon.sock",
      logPath: "/tmp/taugentic-daemon/ta-daemon/ta-daemon.log.jsonl",
      daemonVersion: "0.0.1",
      protocolVersion: "2026-04-stage2",
    });
  });

  it("rejects payloads that drift from the Rust schema", () => {
    expect(() =>
      parseDaemonControlStatusResult({
        backgroundOptIn: "yes",
        desiredMode: "background",
        actualMode: "background",
        transitionStatus: "idle",
        reconcileRequired: false,
        allowedActions: ["stop"],
        message: "Background mode is the desired runtime.",
        socketPath: "/tmp/ta-daemon.sock",
        logPath: "/tmp/taugentic-daemon/ta-daemon/ta-daemon.log.jsonl",
        daemonVersion: "0.0.1",
        protocolVersion: "2026-04-stage2",
      }),
    ).toThrow(ProtocolValidationError);
  });
});

describe("parseDaemonInitializeResult", () => {
  it("accepts Rust-owned daemon initialize payloads", () => {
    expect(
      parseDaemonInitializeResult({
        daemonInstanceId: "daemon-1",
        clientCredential: "credential-1credential-1credential-1",
        daemonVersion: "0.0.1",
        protocolVersion: "2026-04-stage2",
        capabilities: {
          notifications: true,
          eventSubscriptions: true,
        },
      }),
    ).toEqual({
      daemonInstanceId: "daemon-1",
      clientCredential: "credential-1credential-1credential-1",
      daemonVersion: "0.0.1",
      protocolVersion: "2026-04-stage2",
      capabilities: {
        notifications: true,
        eventSubscriptions: true,
      },
    });
  });
});

describe("parseDaemonDiagnostics", () => {
  it("normalizes JSON-round-tripped diagnostics uint64 fields to bigint", () => {
    const parsed = parseDaemonDiagnostics(roundTripJson(makeDaemonDiagnosticsPayload()));

    expect(parsed.uptimeMs).toBe(65_000n);
    expect(parsed.recentErrors[0]!.occurredAtMs).toBe(1_700_000_000_000n);
    expect(parsed.tokenUsage.modelContextWindow).toBe(200_000n);
    expect(parsed.tokenUsage.totalTokens).toBe(12_345n);
    expect(parsed.tokenUsage.promptTokens).toBe(11_000n);
    expect(parsed.tokenUsage.completionTokens).toBe(1_345n);
    expect(parsed.tokenUsage.cachedTokens).toBe(2_000n);
    expect(parsed.tokenUsage.reasoningTokens).toBe(345n);
  });
});

describe("parseDaemonSubscribeResult", () => {
  it("accepts ready subscribe payloads and normalizes latestCursor", () => {
    expect(
      parseDaemonSubscribeResult({
        status: "ready",
        latestCursor: {
          ...daemonCursor("8"),
        },
      }),
    ).toEqual({
      status: "ready",
      latestCursor: {
        ...daemonCursor(8n),
      },
    });
  });

  it("accepts history-gap subscribe payloads and normalizes latestCursor", () => {
    expect(
      parseDaemonSubscribeResult({
        status: "historyGap",
        latestCursor: {
          ...daemonCursor("7"),
        },
      }),
    ).toEqual({
      status: "historyGap",
      latestCursor: {
        ...daemonCursor(7n),
      },
    });
  });
});

function makeDaemonDiagnosticsPayload() {
  return {
    claimCount: 1,
    inFlightCapsuleRunCount: 2,
    inFlightRpcCount: 3,
    providerHealth: [
      {
        displayName: "Codex",
        message: null,
        providerId: "codex",
        status: "ready",
      },
    ],
    recentErrorCount: 1,
    recentErrors: [
      {
        message: "run failed safely",
        occurredAtMs: "1700000000000",
        source: "run",
      },
    ],
    sandbox: {
      appcontainer: false,
      filesystemAllowlist: true,
      helperAvailable: true,
      networkDefaultDeny: true,
      networkDestinationAllowlist: true,
      os: "macos",
      restrictedTokenJob: false,
      sandboxKind: "macos-seatbelt",
    },
    tokenUsage: {
      cachedTokens: "2000",
      completionTokens: "1345",
      modelContextWindow: "200000",
      promptTokens: "11000",
      reasoningTokens: "345",
      totalTokens: "12345",
    },
    uptimeMs: "65000",
    worktreeCount: 4,
  };
}

function roundTripJson(value: unknown): unknown {
  return JSON.parse(JSON.stringify(value)) as unknown;
}
