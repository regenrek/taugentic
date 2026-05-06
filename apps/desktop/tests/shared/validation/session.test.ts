import { describe, expect, it } from "vite-plus/test";

import {
  parseDaemonSessionAttachResult,
  parseDaemonSessionOpenResult,
  parseSessionOverviewResult,
  ProtocolValidationError,
} from "../../../packages/shared/src/validation.js";
import { agentStreamEnvelope, daemonCursor } from "./helpers.js";

describe("parseSessionOverviewResult", () => {
  it("accepts daemon-owned session overview payloads with nested activity envelopes", () => {
    expect(
      parseSessionOverviewResult({
        sessions: [
          {
            session: {
              id: "session-1",
              title: "Build daemon app server",
              status: "running",
            },
            latestRun: {
              id: "run-1",
              objective: "Build daemon app server",
              runtimeProfileId: "runtime-codex-safe",
              status: "waitingForApproval",
            },
            laneStatus: "waitingForApproval",
            isActive: true,
            approvalAttention: "pending",
            pendingApprovalCount: 1,
            lastActivityAtMs: "91",
            lastEventPreview: "Approval requested: execute run",
            recentActivity: [
              {
                daemonInstanceId: "daemon-1",
                sessionId: "session-1",
                sequence: "8",
                occurredAtMs: "91",
                event: {
                  approval: {
                    phase: "requested",
                    request: {
                      id: "approval-1",
                      runId: "run-1",
                      scope: "processExec",
                      requestedAtMs: "90",
                      expiresAtMs: "120000",
                      target: { kind: "processExec", command: "cargo test" },
                      reason: "execute run",
                    },
                  },
                },
              },
            ],
          },
        ],
      }),
    ).toEqual({
      sessions: [
        {
          session: {
            id: "session-1",
            title: "Build daemon app server",
            status: "running",
          },
          latestRun: {
            id: "run-1",
            objective: "Build daemon app server",
            runtimeProfileId: "runtime-codex-safe",
            status: "waitingForApproval",
          },
          laneStatus: "waitingForApproval",
          isActive: true,
          approvalAttention: "pending",
          pendingApprovalCount: 1,
          lastActivityAtMs: 91n,
          lastEventPreview: "Approval requested: execute run",
          recentActivity: [
            {
              daemonInstanceId: "daemon-1",
              sessionId: "session-1",
              sequence: 8n,
              occurredAtMs: 91n,
              event: {
                approval: {
                  phase: "requested",
                  request: {
                    id: "approval-1",
                    runId: "run-1",
                    scope: "processExec",
                    requestedAtMs: "90",
                    expiresAtMs: "120000",
                    target: { kind: "processExec", command: "cargo test" },
                    reason: "execute run",
                  },
                },
              },
            },
          ],
        },
      ],
    });
  });

  it("rejects malformed bigint wire fields in nested session overview activity", () => {
    expect(() =>
      parseSessionOverviewResult({
        sessions: [
          {
            session: {
              id: "session-1",
              title: "Build daemon app server",
              status: "running",
            },
            latestRun: null,
            laneStatus: "idle",
            isActive: false,
            approvalAttention: "idle",
            pendingApprovalCount: 0,
            lastActivityAtMs: "not-a-u64",
            lastEventPreview: null,
            recentActivity: [
              {
                daemonInstanceId: "daemon-1",
                sessionId: "session-1",
                sequence: "8",
                occurredAtMs: "91",
                event: {
                  run: {
                    runId: "run-1",
                    status: "running",
                    detail: "running",
                  },
                },
              },
            ],
          },
        ],
      }),
    ).toThrow(ProtocolValidationError);
  });

  it("accepts agent stream envelopes in nested session overview activity", () => {
    expect(
      parseSessionOverviewResult({
        sessions: [
          {
            session: {
              id: "session-1",
              title: "Build daemon app server",
              status: "running",
            },
            latestRun: null,
            laneStatus: "idle",
            isActive: false,
            approvalAttention: "idle",
            pendingApprovalCount: 0,
            lastActivityAtMs: "101",
            lastEventPreview: "Assistant message delta",
            recentActivity: [
              agentStreamEnvelope({
                kind: "assistantMessageDelta",
                delta: "partial output",
              }),
            ],
          },
        ],
      }),
    ).toEqual({
      sessions: [
        {
          session: {
            id: "session-1",
            title: "Build daemon app server",
            status: "running",
          },
          latestRun: null,
          laneStatus: "idle",
          isActive: false,
          approvalAttention: "idle",
          pendingApprovalCount: 0,
          lastActivityAtMs: 101n,
          lastEventPreview: "Assistant message delta",
          recentActivity: [
            {
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
            },
          ],
        },
      ],
    });
  });
});

describe("parseDaemonSessionAttachResult", () => {
  it("accepts Rust-owned open payloads and normalizes latestCursor", () => {
    expect(
      parseDaemonSessionOpenResult({
        session: {
          id: "session-7",
          title: "Build daemon app server",
          status: "idle",
        },
        sessionAuthority: "session-authority-1session-authority-1",
        latestCursor: {
          ...daemonCursor("40"),
        },
      }),
    ).toEqual({
      session: {
        id: "session-7",
        title: "Build daemon app server",
        status: "idle",
      },
      latestCursor: {
        ...daemonCursor(40n),
      },
      sessionAuthority: "session-authority-1session-authority-1",
    });
  });

  it("accepts Rust-owned attach payloads and normalizes latestCursor", () => {
    expect(
      parseDaemonSessionAttachResult({
        session: {
          id: "session-7",
          title: "Build daemon app server",
          status: "running",
        },
        sessionAuthority: "session-authority-1session-authority-1",
        latestCursor: {
          ...daemonCursor("41"),
        },
      }),
    ).toEqual({
      session: {
        id: "session-7",
        title: "Build daemon app server",
        status: "running",
      },
      latestCursor: {
        ...daemonCursor(41n),
      },
      sessionAuthority: "session-authority-1session-authority-1",
    });
  });

  it("rejects attach payloads that drift from the Rust schema", () => {
    expect(() =>
      parseDaemonSessionAttachResult({
        session: {
          id: "session-7",
          title: "Build daemon app server",
        },
      }),
    ).toThrow(ProtocolValidationError);
  });

  it("rejects open payloads without sessionAuthority", () => {
    expect(() =>
      parseDaemonSessionOpenResult({
        session: {
          id: "session-7",
          title: "Build daemon app server",
          status: "idle",
        },
      }),
    ).toThrow(ProtocolValidationError);
  });
});
