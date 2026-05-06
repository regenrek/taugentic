import { describe, expect, it } from "vite-plus/test";

import type { RunStreamEventEnvelope } from "../../packages/shared/src/ipc.js";
import type { ActivityPageItem } from "../../packages/shared/src/contracts.js";
import {
  createInitialSessionRunState,
  hydrateRunActivity,
  reduceRunStreamMessage,
} from "../../packages/renderer/src/features/runs/state.js";

function makeRunEvent(sequence: bigint): RunStreamEventEnvelope {
  return {
    daemonInstanceId: "daemon-1",
    sessionId: "session-1",
    occurredAtMs: sequence * 10n,
    sequence,
    event: {
      run: {
        detail: `detail-${sequence.toString()}`,
        runId: `run-${sequence.toString()}`,
        status: "running",
      },
    },
  };
}

function makeActivityItem(
  runId: string,
  status: "running" | "completed",
  sequence: bigint,
): ActivityPageItem {
  return {
    cursor: {
      sequence,
    },
    occurredAtMs: sequence * 10n,
    event: {
      run: {
        detail: `detail-${runId}-${status}`,
        runId,
        status,
      },
    },
  };
}

describe("run renderer state helpers", () => {
  it("treats live run events as refresh signals and preserves lifecycle-only state", () => {
    const reduced = reduceRunStreamMessage(
      {
        ...createInitialSessionRunState(),
        isHydrating: false,
      },
      makeRunEvent(4n),
    );

    expect(reduced).toEqual({
      needsRefresh: true,
      state: {
        errorMessage: null,
        isHydrating: false,
        streamStatus: "live",
      },
    });
  });

  it("marks the run stream errored on terminal status envelopes", () => {
    const reduced = reduceRunStreamMessage(createInitialSessionRunState(), {
      stream: "runs",
      status: "terminalError",
    });

    expect(reduced).toEqual({
      needsRefresh: false,
      state: {
        errorMessage: "run stream entered a terminal error state",
        isHydrating: false,
        streamStatus: "error",
      },
    });
  });

  it("hydrates recent run activity from durable activity items and ignores non-run events", () => {
    const hydrated = hydrateRunActivity([
      makeActivityItem("run-3", "completed", 3n),
      {
        cursor: { sequence: 2n },
        occurredAtMs: 20n,
        event: {
          approval: {
            phase: "requested",
            request: {
              expiresAtMs: 60_000n,
              id: "approval-1",
              requestedAtMs: 0n,
              runId: "run-1",
              reason: "needs review",
              scope: "fileWrite",
              target: { kind: "fileWrite", paths: ["src/main.rs"] },
            },
          },
        },
      },
      makeActivityItem("run-2", "running", 2n),
    ]);

    expect(hydrated).toEqual([
      {
        cursor: {
          sequence: 3n,
        },
        event: {
          run: {
            detail: "detail-run-3-completed",
            runId: "run-3",
            status: "completed",
          },
        },
        occurredAtMs: 30n,
      },
      {
        cursor: {
          sequence: 2n,
        },
        event: {
          run: {
            detail: "detail-run-2-running",
            runId: "run-2",
            status: "running",
          },
        },
        occurredAtMs: 20n,
      },
    ]);
  });

  it("keeps distinct run activity items when only the durable cursor differs", () => {
    const hydrated = hydrateRunActivity([
      makeActivityItem("run-7", "completed", 7n),
      {
        cursor: {
          sequence: 8n,
        },
        occurredAtMs: 70n,
        event: {
          run: {
            detail: "detail-run-7-completed",
            runId: "run-7",
            status: "completed",
          },
        },
      },
    ]);

    expect(hydrated).toEqual([
      {
        cursor: {
          sequence: 7n,
        },
        event: {
          run: {
            detail: "detail-run-7-completed",
            runId: "run-7",
            status: "completed",
          },
        },
        occurredAtMs: 70n,
      },
      {
        cursor: {
          sequence: 8n,
        },
        event: {
          run: {
            detail: "detail-run-7-completed",
            runId: "run-7",
            status: "completed",
          },
        },
        occurredAtMs: 70n,
      },
    ]);
  });
});
