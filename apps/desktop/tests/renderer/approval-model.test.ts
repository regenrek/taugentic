import { describe, expect, it, vi } from "vite-plus/test";

import type { ApprovalSnapshotResult } from "../../packages/shared/generated/index.js";
import { createSessionApprovalActor } from "../../packages/renderer/src/features/approvals/connection.js";
import { selectSessionApprovalViewState } from "../../packages/renderer/src/features/approvals/model.js";

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;

  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });

  return { promise, reject, resolve };
}

function createFakePort() {
  return vi.fn(async () => () => undefined);
}

async function flushMicrotasks(turns = 16): Promise<void> {
  for (let index = 0; index < turns; index += 1) {
    await Promise.resolve();
  }
}

describe("approval model", () => {
  it("tracks pending approval decisions and clears them after success", async () => {
    const deferredDecision = createDeferred<void>();
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(() => deferredDecision.promise),
        hydrateSnapshot: vi.fn(),
        listApprovals: vi.fn(async () => ({ items: [], latestCursor: null })),
        subscribeApprovalStream: createFakePort(),
      },
      sessionId: "session-1",
    });

    actor.start();
    await flushMicrotasks();
    actor.send({
      type: "approvalDecisionRequested",
      approvalId: "approval-1",
      decision: "approved",
    });
    await flushMicrotasks();

    expect(selectSessionApprovalViewState(actor.getSnapshot())).toMatchObject({
      commandErrorMessage: null,
      pendingApprovalId: "approval-1",
      pendingDecision: "approved",
    });

    deferredDecision.resolve();
    await flushMicrotasks();

    expect(selectSessionApprovalViewState(actor.getSnapshot())).toMatchObject({
      commandErrorMessage: null,
      pendingApprovalId: null,
      pendingDecision: null,
    });
  });

  it("surfaces command failures without disturbing lifecycle state", async () => {
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval: vi.fn(async () => {
          throw new Error("approval rejected locally");
        }),
        hydrateSnapshot: vi.fn(),
        listApprovals: vi.fn(
          async () =>
            ({
              items: [
                {
                  expiresAtMs: 60_000n,
                  id: "approval-1",
                  reason: "need shell",
                  requestedAtMs: 0n,
                  runId: "run-1",
                  scope: "processExec",
                  target: { kind: "processExec", command: "echo ok" },
                },
              ],
              latestCursor: null,
            }) satisfies ApprovalSnapshotResult,
        ),
        subscribeApprovalStream: createFakePort(),
      },
      sessionId: "session-1",
    });

    actor.start();
    await flushMicrotasks();
    actor.send({
      type: "approvalDecisionRequested",
      approvalId: "approval-1",
      decision: "rejected",
    });
    await flushMicrotasks();

    expect(selectSessionApprovalViewState(actor.getSnapshot())).toMatchObject({
      commandErrorMessage: "approval rejected locally",
      pendingApprovalId: null,
      pendingDecision: null,
    });
  });

  it("keeps the latest approval decision authoritative when an older request settles late", async () => {
    const firstDecision = createDeferred<void>();
    const secondDecision = createDeferred<void>();
    const decideApproval = vi
      .fn<
        (sessionId: string, approvalId: string, decision: "approved" | "rejected") => Promise<void>
      >()
      .mockImplementationOnce(() => firstDecision.promise)
      .mockImplementationOnce(() => secondDecision.promise);
    const actor = createSessionApprovalActor({
      deps: {
        decideApproval,
        hydrateSnapshot: vi.fn(),
        listApprovals: vi.fn(async () => ({ items: [], latestCursor: null })),
        subscribeApprovalStream: createFakePort(),
      },
      sessionId: "session-1",
    });

    actor.start();
    await flushMicrotasks();
    actor.send({
      type: "approvalDecisionRequested",
      approvalId: "approval-1",
      decision: "approved",
    });
    await flushMicrotasks();

    expect(selectSessionApprovalViewState(actor.getSnapshot())).toMatchObject({
      pendingApprovalId: "approval-1",
      pendingDecision: "approved",
    });

    actor.send({
      type: "approvalDecisionRequested",
      approvalId: "approval-2",
      decision: "rejected",
    });
    await flushMicrotasks();

    expect(selectSessionApprovalViewState(actor.getSnapshot())).toMatchObject({
      pendingApprovalId: "approval-2",
      pendingDecision: "rejected",
    });

    secondDecision.resolve();
    await flushMicrotasks();
    firstDecision.reject(new Error("stale failure"));
    await flushMicrotasks();

    expect(selectSessionApprovalViewState(actor.getSnapshot())).toMatchObject({
      commandErrorMessage: null,
      pendingApprovalId: null,
      pendingDecision: null,
    });
  });
});
