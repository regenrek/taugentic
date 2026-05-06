import { describe, expect, it } from "vite-plus/test";

import type {
  PublicDaemonEventEnvelope,
  SessionOverview,
  SessionOverviewResult,
} from "../../packages/shared/generated/index.js";
import {
  boundedMerge,
  eventKind,
  formatEventSummary,
  mergeRecentActivity,
} from "../../packages/renderer/src/features/activity-log/formatters.js";

function makeEnvelope(
  partial: Partial<PublicDaemonEventEnvelope> & Pick<PublicDaemonEventEnvelope, "event">,
): PublicDaemonEventEnvelope {
  return {
    daemonInstanceId: "daemon-1",
    occurredAtMs: 100n,
    sequence: 1n,
    sessionId: "session-a",
    ...partial,
  };
}

function makeOverview(id: string, activity: PublicDaemonEventEnvelope[]): SessionOverview {
  return {
    approvalAttention: "idle",
    isActive: false,
    laneStatus: "idle",
    pendingApprovalCount: 0,
    recentActivity: activity,
    session: { id, status: "idle", title: id },
  };
}

describe("activity log formatters", () => {
  it("flattens, dedupes by (daemonInstanceId, sessionId, sequence), and sorts desc", () => {
    const envelopes = {
      a1: makeEnvelope({
        event: { session: { sessionId: "session-a", status: "running" } },
        occurredAtMs: 100n,
        sequence: 1n,
        sessionId: "session-a",
      }),
      a1Duplicate: makeEnvelope({
        event: { session: { sessionId: "session-a", status: "running" } },
        occurredAtMs: 100n,
        sequence: 1n,
        sessionId: "session-a",
      }),
      a2: makeEnvelope({
        event: { session: { sessionId: "session-a", status: "running" } },
        occurredAtMs: 300n,
        sequence: 2n,
        sessionId: "session-a",
      }),
      b1: makeEnvelope({
        event: { session: { sessionId: "session-b", status: "running" } },
        occurredAtMs: 200n,
        sequence: 1n,
        sessionId: "session-b",
      }),
    };
    const result: SessionOverviewResult = {
      sessions: [
        makeOverview("session-a", [envelopes.a1, envelopes.a1Duplicate, envelopes.a2]),
        makeOverview("session-b", [envelopes.b1]),
      ],
    };

    const merged = mergeRecentActivity(result);

    expect(merged.map((envelope) => envelope.occurredAtMs)).toEqual([300n, 200n, 100n]);
    expect(merged.length).toBe(3);
    expect(merged[0]!.sessionId).toBe("session-a");
    expect(merged[0]!.sequence).toBe(2n);
    expect(merged[1]!.sessionId).toBe("session-b");
  });

  it("trims the bounded merge to the provided max", () => {
    const existing: PublicDaemonEventEnvelope[] = [];
    const incoming: PublicDaemonEventEnvelope[] = [];
    for (let index = 0; index < 150; index += 1) {
      existing.push(
        makeEnvelope({
          event: { session: { sessionId: "session-a", status: "running" } },
          occurredAtMs: BigInt(index),
          sequence: BigInt(index),
          sessionId: "session-a",
        }),
      );
    }
    for (let index = 150; index < 250; index += 1) {
      incoming.push(
        makeEnvelope({
          event: { session: { sessionId: "session-a", status: "running" } },
          occurredAtMs: BigInt(index),
          sequence: BigInt(index),
          sessionId: "session-a",
        }),
      );
    }

    const merged = boundedMerge(existing, incoming, 100);

    expect(merged.length).toBe(100);
    expect(merged[0]!.sequence).toBe(249n);
    expect(merged[99]!.sequence).toBe(150n);
  });

  it("classifies each canonical event shape into the right kind", () => {
    expect(
      eventKind(
        makeEnvelope({
          event: { session: { sessionId: "session-a", status: "running" } },
        }),
      ),
    ).toBe("session");

    expect(
      eventKind(
        makeEnvelope({
          event: { run: { runId: "run-1", status: "running", detail: "" } },
        }),
      ),
    ).toBe("run");

    expect(
      eventKind(
        makeEnvelope({
          event: {
            approval: {
              phase: "requested",
              request: {
                id: "approval-1",
                runId: "run-1",
                scope: "networkAccess",
                requestedAtMs: 100n,
                expiresAtMs: 120000n,
                target: { kind: "networkAccess", host: "api.example.com", protocol: "https" },
                reason: "access",
              },
            },
          },
        }),
      ),
    ).toBe("approval");

    expect(
      eventKind(
        makeEnvelope({
          event: {
            artifact: {
              artifact: {
                id: "artifact-1",
                runId: "run-1",
                kind: "CommandLog",
                storagePath: "/tmp/out.log",
              },
            },
          },
        }),
      ),
    ).toBe("artifact");
  });

  it("summarises each canonical event shape as a non-empty single line", () => {
    const summaries = [
      formatEventSummary(
        makeEnvelope({ event: { session: { sessionId: "session-a", status: "running" } } }),
      ),
      formatEventSummary(
        makeEnvelope({
          event: { run: { runId: "run-1", status: "completed", detail: "ok" } },
        }),
      ),
      formatEventSummary(
        makeEnvelope({
          event: {
            approval: {
              phase: "requested",
              request: {
                id: "approval-1",
                runId: "run-1",
                scope: "networkAccess",
                requestedAtMs: 100n,
                expiresAtMs: 120000n,
                target: { kind: "networkAccess", host: "api.example.com", protocol: "https" },
                reason: "needs access",
              },
            },
          },
        }),
      ),
      formatEventSummary(
        makeEnvelope({
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
      ),
      formatEventSummary(
        makeEnvelope({
          event: {
            artifact: {
              artifact: {
                id: "artifact-1",
                runId: "run-1",
                kind: "CommandLog",
                storagePath: "/tmp/out.log",
              },
            },
          },
        }),
      ),
    ];

    for (const summary of summaries) {
      expect(summary.length).toBeGreaterThan(0);
      expect(summary.includes("\n")).toBe(false);
    }
  });
});
