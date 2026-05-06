import { describe, expect, it } from "vite-plus/test";

import {
  applyApprovalStreamError,
  applyApprovalStreamMessage,
} from "../../packages/renderer/src/features/session-detail/useSessionApprovalLiveSync.js";
import { createInitialSessionApprovalState } from "../../packages/renderer/src/features/approvals/stream-state.js";
import type { ApprovalStreamMessage } from "../../packages/shared/src/ipc.js";

function approvalEnvelope(sequence: bigint): ApprovalStreamMessage {
  return {
    daemonInstanceId: "daemon-1",
    sessionId: "session-1",
    sequence,
    occurredAtMs: 100n,
    event: {
      approval: {
        phase: "requested" as const,
        request: {
          expiresAtMs: 60_000n,
          id: "approval-1",
          requestedAtMs: 0n,
          runId: "run-1",
          scope: "fileWrite" as const,
          target: { kind: "fileWrite" as const, paths: ["src/main.rs"] },
          reason: "Write src/main.rs",
        },
      },
    },
  };
}

describe("applyApprovalStreamMessage", () => {
  it("returns shouldInvalidate=true for live approval envelopes and advances cursor", () => {
    const initial = {
      ...createInitialSessionApprovalState("session-1"),
      streamStatus: "ready" as const,
    };
    const step = applyApprovalStreamMessage(initial, approvalEnvelope(42n));

    expect(step.shouldInvalidate).toBe(true);
    expect(step.nextView.streamStatus).toBe("ready");
    expect(step.nextView.lastSequence).toBe(42n);
    expect(step.nextState.lastSequence).toBe(42n);
  });

  it("returns shouldInvalidate=false for 'ready' lifecycle envelopes", () => {
    const step = applyApprovalStreamMessage(createInitialSessionApprovalState("session-1"), {
      stream: "approvals",
      status: "ready",
    });

    expect(step.shouldInvalidate).toBe(false);
    expect(step.nextView.streamStatus).toBe("ready");
    expect(step.nextView.errorMessage).toBeNull();
  });

  it("returns shouldInvalidate=true for 'historyGap' so the query refetches", () => {
    const step = applyApprovalStreamMessage(createInitialSessionApprovalState("session-1"), {
      stream: "approvals",
      status: "historyGap",
    });

    expect(step.shouldInvalidate).toBe(true);
    expect(step.nextView.streamStatus).toBe("ready");
  });

  it("marks the stream as terminal error without triggering a refetch", () => {
    const step = applyApprovalStreamMessage(createInitialSessionApprovalState("session-1"), {
      stream: "approvals",
      status: "terminalError",
    });

    expect(step.shouldInvalidate).toBe(false);
    expect(step.nextView.streamStatus).toBe("error");
    expect(step.nextView.errorMessage).toContain("session-1");
  });
});

describe("applyApprovalStreamError", () => {
  it("sets streamStatus=error with a lane-agnostic error message", () => {
    const step = applyApprovalStreamError(
      createInitialSessionApprovalState("session-1"),
      "session-1",
      new Error("socket closed"),
    );

    expect(step.shouldInvalidate).toBe(false);
    expect(step.nextView.streamStatus).toBe("error");
    expect(step.nextView.errorMessage).toContain("session-1");
    expect(step.nextView.errorMessage).toContain("socket closed");
  });

  it("preserves the prior lastSequence on error", () => {
    const priorStep = applyApprovalStreamMessage(
      createInitialSessionApprovalState("session-1"),
      approvalEnvelope(17n),
    );
    const errorStep = applyApprovalStreamError(priorStep.nextState, "session-1", "timeout");
    expect(errorStep.nextView.lastSequence).toBe(17n);
  });

  it("does not branch on runtime family — same shape for native / ACP / codex-app-server errors", () => {
    const acp = applyApprovalStreamError(
      createInitialSessionApprovalState("session-1"),
      "session-1",
      new Error("acp adapter hung up"),
    );
    const codexApp = applyApprovalStreamError(
      createInitialSessionApprovalState("session-1"),
      "session-1",
      new Error("codex-app-server disconnected"),
    );
    expect(acp.nextView.streamStatus).toBe(codexApp.nextView.streamStatus);
    expect(acp.shouldInvalidate).toBe(codexApp.shouldInvalidate);
  });
});
