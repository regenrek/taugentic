import { describe, expect, it } from "vite-plus/test";

import {
  createInitialSessionApprovalState,
  reduceApprovalStreamMessage,
  toApprovalStreamErrorMessage,
} from "../../packages/renderer/src/features/approvals/stream-state.js";

describe("approval stream state", () => {
  it("marks the session stream ready on ready envelopes", () => {
    expect(
      reduceApprovalStreamMessage(createInitialSessionApprovalState("session-1"), {
        stream: "approvals",
        status: "ready",
      }),
    ).toEqual({
      needsRefresh: false,
      state: {
        errorMessage: null,
        lastSequence: null,
        sessionId: "session-1",
        streamStatus: "ready",
      },
    });
  });

  it("treats live approval events as refresh signals and preserves lifecycle-only state", () => {
    const initial = {
      ...createInitialSessionApprovalState("session-1"),
      streamStatus: "ready" as const,
    };

    expect(
      reduceApprovalStreamMessage(initial, {
        daemonInstanceId: "daemon-1",
        sessionId: "session-1",
        sequence: 41n,
        occurredAtMs: 99n,
        event: {
          approval: {
            phase: "requested",
            request: {
              expiresAtMs: 60_000n,
              id: "approval-1",
              requestedAtMs: 0n,
              runId: "run-1",
              scope: "processExec",
              target: { kind: "processExec", command: "echo ok" },
              reason: "needs approval",
            },
          },
        },
      }),
    ).toEqual({
      needsRefresh: true,
      state: {
        errorMessage: null,
        lastSequence: 41n,
        sessionId: "session-1",
        streamStatus: "ready",
      },
    });
  });

  it("marks the approval stream errored on terminal status envelopes", () => {
    expect(
      reduceApprovalStreamMessage(createInitialSessionApprovalState("session-1"), {
        stream: "approvals",
        status: "terminalError",
      }),
    ).toEqual({
      needsRefresh: false,
      state: {
        errorMessage: "approval stream entered a terminal error state for session-1",
        lastSequence: null,
        sessionId: "session-1",
        streamStatus: "error",
      },
    });
  });

  it("formats deterministic session-scoped stream errors", () => {
    expect(toApprovalStreamErrorMessage("session-9", new Error("boom"))).toBe(
      "approval stream failed for session-9: boom",
    );
  });
});
