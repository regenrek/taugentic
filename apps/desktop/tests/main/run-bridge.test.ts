import { describe, expect, it, vi } from "vite-plus/test";

import { DaemonSessionRequestClient } from "../../packages/main/src/daemon-session-request-client.js";

type TestableRequestClient = {
  connection: {
    request: (
      method: string,
      params: Record<string, unknown>,
      parseResult: (value: unknown) => unknown,
    ) => Promise<unknown>;
  };
  ensureConnected: () => Promise<void>;
  getRunDetail: DaemonSessionRequestClient["getRunDetail"];
  listNativeRuns: DaemonSessionRequestClient["listNativeRuns"];
};

describe("native run bridge", () => {
  it("passes list_native lineage fields through from daemon wire result", async () => {
    const daemonResult = {
      runs: [
        {
          id: "run-child",
          parentRunId: "run-parent",
          outputContract: "review",
          recipeId: "review-native-subagent",
          harness: "native",
          status: "running",
          startedAtMs: "120",
          endedAtMs: null,
          lastEventSeq: "42",
          objectivePreview: "Review native child",
        },
      ],
      nextCursor: "120:run-child",
    };
    const session = new DaemonSessionRequestClient("session-1") as unknown as TestableRequestClient;

    session.ensureConnected = vi.fn(async () => {});
    const request = vi.fn(
      async (
        _method: string,
        _params: Record<string, unknown>,
        parseResult: (value: unknown) => unknown,
      ) => parseResult(daemonResult),
    );
    session.connection.request = request;

    await expect(session.listNativeRuns({ limit: 25 })).resolves.toEqual(daemonResult);
    expect(request).toHaveBeenCalledWith(
      "daemon.run.list_native",
      { limit: 25 },
      expect.any(Function),
    );
  });

  it("passes daemon.run.get RunDetail fields through from daemon wire result", async () => {
    const daemonResult = {
      summary: {
        id: "run-child",
        runtimeProfileId: "runtime-openai-safe",
        objective: "Review native child",
        status: "failed",
      },
      contractViolation: {
        kind: "kindMismatch",
        value: {
          expected: "review",
          got: "patch",
        },
      },
      quarantineReceipt: {
        id: "receipt-quarantine",
        sessionId: "session-1",
        runId: "run-child",
        parentRunId: "run-parent",
        kind: "reviewFinding",
        provenance: {
          streamCursor: "run:run-child:event:42",
        },
        state: "quarantined",
        summary: "Review CapsuleResult quarantined after daemon validation",
        createdAtMs: "120",
        quarantinedAtMs: "121",
      },
      outputContract: "review",
      recipeId: "review-native-subagent",
      parentRunId: "run-parent",
    };
    const session = new DaemonSessionRequestClient("session-1") as unknown as TestableRequestClient;

    session.ensureConnected = vi.fn(async () => {});
    const request = vi.fn(
      async (
        _method: string,
        _params: Record<string, unknown>,
        parseResult: (value: unknown) => unknown,
      ) => parseResult(daemonResult),
    );
    session.connection.request = request;

    await expect(session.getRunDetail("run-child")).resolves.toEqual(daemonResult);
    expect(request).toHaveBeenCalledWith(
      "daemon.run.get",
      { runId: "run-child" },
      expect.any(Function),
    );
  });
});
