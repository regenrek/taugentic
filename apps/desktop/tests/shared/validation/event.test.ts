import { describe, expect, it } from "vite-plus/test";

import {
  parseDaemonEventEnvelope,
  ProtocolValidationError,
} from "../../../packages/shared/src/validation.js";
import { agentStreamEnvelope } from "./helpers.js";

describe("parseDaemonEventEnvelope", () => {
  it("accepts artifact events and normalizes transport fields to bigint", () => {
    expect(
      parseDaemonEventEnvelope({
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: "42",
        occurredAtMs: "99",
        event: {
          artifact: {
            artifact: {
              id: "artifact-1",
              runId: "run-1",
              kind: "Patch",
              storagePath: "artifacts/run-1/patch.diff",
            },
          },
        },
      }),
    ).toEqual({
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 42n,
      occurredAtMs: 99n,
      event: {
        artifact: {
          artifact: {
            id: "artifact-1",
            runId: "run-1",
            kind: "Patch",
            storagePath: "artifacts/run-1/patch.diff",
          },
        },
      },
    });
  });

  it("normalizes decimal-string u64 transport fields to bigint", () => {
    expect(
      parseDaemonEventEnvelope({
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: "42",
        occurredAtMs: "99",
        event: {
          run: {
            runId: "run-1",
            status: "running",
            detail: "running",
          },
        },
      }),
    ).toEqual({
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 42n,
      occurredAtMs: 99n,
      event: {
        run: {
          runId: "run-1",
          status: "running",
          detail: "running",
        },
      },
    });
  });

  it("rejects numeric transport fields that drift from the wire contract", () => {
    expect(() =>
      parseDaemonEventEnvelope({
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 42,
        occurredAtMs: "99",
        event: {
          run: {
            runId: "run-1",
            status: "running",
            detail: "running",
          },
        },
      }),
    ).toThrow(ProtocolValidationError);
  });

  it("rejects decimal strings above the Rust uint64 range", () => {
    expect(() =>
      parseDaemonEventEnvelope({
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: "18446744073709551616",
        occurredAtMs: "99",
        event: {
          run: {
            runId: "run-1",
            status: "running",
            detail: "running",
          },
        },
      }),
    ).toThrow(/DaemonEventEnvelope.sequence must be <= 18446744073709551615/);
  });

  it("rejects overlong uint64 strings before bigint parsing", () => {
    expect(() =>
      parseDaemonEventEnvelope({
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: "184467440737095516150",
        occurredAtMs: "99",
        event: {
          run: {
            runId: "run-1",
            status: "queued",
            detail: "queued",
          },
        },
      }),
    ).toThrow(ProtocolValidationError);
  });

  it("accepts resolved approval events without public actor or commentary", () => {
    expect(
      parseDaemonEventEnvelope({
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: "43",
        occurredAtMs: "100",
        event: {
          approval: {
            phase: "resolved",
            resolution: {
              approvalId: "approval-1",
              runId: "run-1",
              decision: "approved",
              reason: "user",
            },
          },
        },
      }),
    ).toEqual({
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 43n,
      occurredAtMs: 100n,
      event: {
        approval: {
          phase: "resolved",
          resolution: {
            approvalId: "approval-1",
            runId: "run-1",
            decision: "approved",
            reason: "user",
          },
        },
      },
    });
  });

  it("rejects resolved approval events that leak public actor or commentary", () => {
    expect(() =>
      parseDaemonEventEnvelope({
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: "43",
        occurredAtMs: "100",
        event: {
          approval: {
            phase: "resolved",
            resolution: {
              approvalId: "approval-1",
              runId: "run-1",
              decision: "approved",
              reason: "user",
              actor: {
                principalId: "principal-1",
              },
              commentary: "looks safe",
            },
          },
        },
      }),
    ).toThrow(ProtocolValidationError);
  });

  it("accepts assistant message delta agent stream events", () => {
    expect(
      parseDaemonEventEnvelope(
        agentStreamEnvelope({
          kind: "assistantMessageDelta",
          delta: "partial output",
        }),
      ),
    ).toEqual({
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 44n,
      occurredAtMs: 101n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: "item-1",
          fragmentSequence: 3,
          frame: {
            kind: "assistantMessageDelta",
            delta: "partial output",
          },
        },
      },
    });
  });

  it("accepts tool call started agent stream events with toolName", () => {
    expect(
      parseDaemonEventEnvelope(
        agentStreamEnvelope({
          kind: "toolCallStarted",
          toolName: "shell",
          input: '{"cmd":"echo hi"}',
        }),
      ),
    ).toEqual({
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 44n,
      occurredAtMs: 101n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: "item-1",
          fragmentSequence: 3,
          frame: {
            kind: "toolCallStarted",
            toolName: "shell",
            input: '{"cmd":"echo hi"}',
          },
        },
      },
    });
  });

  it("accepts pending state changed agent stream events", () => {
    expect(
      parseDaemonEventEnvelope(
        agentStreamEnvelope({
          kind: "pendingStateChanged",
          state: "waitingForApproval",
        }),
      ),
    ).toEqual({
      daemonInstanceId: "daemon-1",
      sessionId: "session-1",
      sequence: 44n,
      occurredAtMs: 101n,
      event: {
        agentStream: {
          runId: "run-1",
          turnId: "turn-1",
          itemId: "item-1",
          fragmentSequence: 3,
          frame: {
            kind: "pendingStateChanged",
            state: "waitingForApproval",
          },
        },
      },
    });
  });

  it("rejects drifted toolCallStarted agent stream events with tool_name", () => {
    expect(() =>
      parseDaemonEventEnvelope(
        agentStreamEnvelope({
          kind: "toolCallStarted",
          input: '{"cmd":"echo hi"}',
          tool_name: "shell",
        }),
      ),
    ).toThrow(ProtocolValidationError);
  });
});
