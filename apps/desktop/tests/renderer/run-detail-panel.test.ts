import { describe, expect, it } from "vite-plus/test";

import type {
  CapsuleResult,
  ContextReceipt,
  RunDetail,
  RunEventDelta,
  RunListEntry,
  ValidationError,
} from "../../packages/shared/src/contracts.js";
import { RunDetailPanelView } from "../../packages/renderer/src/features/run-tree/index.js";
import type { RunConflictWarningItem } from "../../packages/renderer/src/lib/queries/session-queries.js";

type CreateElementFn = (
  component: unknown,
  props?: Record<string, unknown> | null,
  ...children: unknown[]
) => unknown;
type RenderToStaticMarkupFn = (element: unknown) => string;

const reactModulePath = "../../packages/renderer/node_modules/react/index.js";
const reactServerModulePath = "../../packages/renderer/node_modules/react-dom/server.node.js";

const { createElement } = (await import(reactModulePath)) as {
  createElement: CreateElementFn;
};
const { renderToStaticMarkup } = (await import(reactServerModulePath)) as {
  renderToStaticMarkup: RenderToStaticMarkupFn;
};

function renderPanel(
  run: RunListEntry | null,
  detail: RunDetail | null,
  activeTab?: string,
  conflictWarnings: RunConflictWarningItem[] = [],
  timelineEvents: RunEventDelta[] = [],
): string {
  return renderToStaticMarkup(
    createElement(RunDetailPanelView, {
      activeTab,
      conflictWarnings,
      detail,
      run,
      timelineEvents,
    }),
  );
}

function makeRun(overrides: Partial<RunListEntry> = {}): RunListEntry {
  return {
    id: "run-patch-123456",
    harness: "native",
    outputContract: "patch",
    recipeId: null,
    status: "completed",
    ...overrides,
  };
}

function makePatchResult(): CapsuleResult {
  return {
    kind: "patch",
    value: {
      blockers: [],
      passing: true,
      patchReceiptIds: ["receipt-patch"],
      testsRunReceiptIds: ["receipt-test"],
      touchedFiles: ["apps/desktop/packages/renderer/src/features/run-tree/RunDetailPanel.tsx"],
    },
  };
}

function makeDetail(overrides: Partial<RunDetail> = {}): RunDetail {
  return {
    contractViolation: null,
    outputContract: "patch",
    parentRunId: null,
    quarantineReceipt: null,
    recipeId: null,
    result: makePatchResult(),
    summary: {
      id: "run-patch-123456",
      objective: "Patch run",
      runtimeProfileId: "runtime-openai-safe",
      status: "completed",
    },
    ...overrides,
  };
}

function makeDetailForRun(run: RunListEntry, overrides: Partial<RunDetail> = {}): RunDetail {
  return makeDetail({
    outputContract: run.outputContract ?? null,
    parentRunId: run.parentRunId ?? null,
    recipeId: run.recipeId ?? null,
    summary: {
      id: run.id,
      objective: run.objectivePreview ?? run.id,
      runtimeProfileId: "runtime-openai-safe",
      status: run.status,
    },
    ...overrides,
  });
}

describe("RunDetailPanel", () => {
  it("renders no panel content when selectedRunId is null", () => {
    expect(renderPanel(null, null)).toBe("");
  });

  it("renders a valid PatchResult as pretty JSON in the Result tab", () => {
    const markup = renderPanel(makeRun(), makeDetail());

    expect(markup).toContain("CapsuleResult");
    expect(markup).toContain("&quot;kind&quot;: &quot;patch&quot;");
    expect(markup).toContain("&quot;touchedFiles&quot;");
    expect(markup).toContain("RunDetailPanel.tsx");
  });

  it("renders a visible Violation tab with ValidationError fields", () => {
    const validationError: ValidationError = {
      kind: "testCountsInconsistent",
      value: {
        sumOfParts: 4,
        total: 3,
      },
    };
    const markup = renderPanel(
      makeRun({ outputContract: "test", status: "failed" }),
      makeDetail({
        contractViolation: validationError,
        outputContract: "test",
        result: null,
        summary: {
          id: "run-patch-123456",
          objective: "Test run",
          runtimeProfileId: "runtime-openai-safe",
          status: "failed",
        },
      }),
      "violation",
    );

    expect(markup).toContain(">Violation<");
    expect(markup).toContain("testCountsInconsistent");
    expect(markup).toContain("&quot;sumOfParts&quot;: 4");
  });

  it("renders a visible Quarantine tab with receipt provenance and reason", () => {
    const receipt: ContextReceipt = {
      createdAtMs: 100n,
      id: "receipt-quarantine",
      kind: "patch",
      parentRunId: "run-parent",
      provenance: {
        streamCursor: "run:run-patch-123456:event:42",
      },
      runId: "run-patch-123456",
      sessionId: "session-1",
      state: "quarantined",
      summary: "Patch CapsuleResult quarantined after daemon validation",
    };
    const markup = renderPanel(
      makeRun({ status: "failed" }),
      makeDetail({
        quarantineReceipt: receipt,
        contractViolation: {
          kind: "custom",
          value: "validation failed",
        },
      }),
      "quarantine",
    );

    expect(markup).toContain(">Quarantine<");
    expect(markup).toContain("receipt-quarantine");
    expect(markup).toContain("Patch CapsuleResult quarantined after daemon validation");
    expect(markup).toContain("run:run-patch-123456:event:42");
  });

  it("renders the expected Result empty state for a running run without result", () => {
    const markup = renderPanel(
      makeRun({ outputContract: "patch", status: "running" }),
      makeDetail({
        result: null,
        contractViolation: null,
        summary: {
          id: "run-patch-123456",
          objective: "Running run",
          runtimeProfileId: "runtime-openai-safe",
          status: "running",
        },
      }),
    );

    expect(markup).toContain("no valid result available for this run");
    expect(markup).not.toContain(">Violation<");
    expect(markup).not.toContain(">Quarantine<");
  });

  it("keeps header badges and conditional tabs aligned for run outcome states", () => {
    const baseRun = makeRun({
      id: "run-status-matrix",
      objectivePreview: "Status matrix",
      outputContract: "patch",
    });
    const cases = [
      {
        activeTab: "result",
        detail: makeDetailForRun({ ...baseRun, status: "running" }, { result: null }),
        expectedBadge: "running",
        expectedText: "no valid result available for this run",
        run: { ...baseRun, status: "running" },
        visibleTabs: ["Result", "Membrane", "Logs", "Timeline", "Raw"],
      },
      {
        activeTab: "result",
        detail: makeDetailForRun({ ...baseRun, status: "completed" }),
        expectedBadge: "completed",
        expectedText: "CapsuleResult",
        run: { ...baseRun, status: "completed" },
        visibleTabs: ["Result", "Membrane", "Logs", "Timeline", "Raw"],
      },
      {
        activeTab: "result",
        detail: makeDetailForRun({ ...baseRun, status: "failed" }, { result: null }),
        expectedBadge: "failed",
        expectedText: "no valid result available for this run",
        run: { ...baseRun, status: "failed" },
        visibleTabs: ["Result", "Membrane", "Logs", "Timeline", "Raw"],
      },
      {
        activeTab: "violation",
        detail: makeDetailForRun(
          { ...baseRun, status: "failed" },
          {
            contractViolation: {
              kind: "kindMismatch",
              value: {
                expected: "patch",
                got: "review",
              },
            },
            result: null,
          },
        ),
        expectedBadge: "failed",
        expectedText: "kindMismatch",
        run: { ...baseRun, status: "failed" },
        visibleTabs: ["Result", "Membrane", "Logs", "Timeline", "Violation", "Raw"],
      },
      {
        activeTab: "quarantine",
        detail: makeDetailForRun(
          { ...baseRun, status: "failed" },
          {
            quarantineReceipt: {
              createdAtMs: 200n,
              id: "receipt-quarantine-matrix",
              kind: "patch",
              parentRunId: null,
              provenance: {
                streamCursor: "run:run-status-matrix:event:7",
              },
              runId: "run-status-matrix",
              sessionId: "session-1",
              state: "quarantined",
              summary: "Result quarantined by contract validation",
            },
            result: null,
          },
        ),
        expectedBadge: "failed",
        expectedText: "receipt-quarantine-matrix",
        run: { ...baseRun, status: "failed" },
        visibleTabs: ["Result", "Membrane", "Logs", "Timeline", "Quarantine", "Raw"],
      },
    ] satisfies Array<{
      activeTab: string;
      detail: RunDetail;
      expectedBadge: string;
      expectedText: string;
      run: RunListEntry;
      visibleTabs: string[];
    }>;

    for (const testCase of cases) {
      const markup = renderPanel(testCase.run, testCase.detail, testCase.activeTab);

      expect(markup).toContain(`>${testCase.expectedBadge}<`);
      expect(markup).toContain(testCase.expectedText);
      for (const tab of [
        "Result",
        "Membrane",
        "Logs",
        "Timeline",
        "Violation",
        "Quarantine",
        "Raw",
      ]) {
        const shouldRender = testCase.visibleTabs.includes(tab);
        expect(markup.includes(`>${tab}<`)).toBe(shouldRender);
      }
    }
  });

  it("renders the recipe tag in the header when recipeId is set", () => {
    const markup = renderPanel(makeRun({ recipeId: "debug-agent" }), makeDetail());

    expect(markup).toContain("debug-agent");
    expect(markup).toContain('title="debug-agent"');
  });

  it("renders workspace details, claims, and timestamped conflict warnings", () => {
    const run = makeRun({
      claimedFiles: ["apps/desktop/package.json"],
      conflictSummary: {
        files: ["apps/desktop/package.json"],
        warningCount: 1,
      },
      workspaceInfo: {
        branch: "ta/capsule-run-child-b",
        cleanupPolicy: "deleteOnSuccess",
        path: "/tmp/taugentic-worktrees/run-child-b",
      },
    });
    const detail = makeDetailForRun(run, {
      tokenUsage: {
        promptTokens: 11_000n,
        completionTokens: 1_345n,
        cachedTokens: 2_000n,
        reasoningTokens: 345n,
      },
    });
    const warning = {
      occurredAtMs: 1_700_000_000_000n,
      runId: run.id,
      warning: {
        conflicts: [
          {
            file: "apps/desktop/package.json",
            holdingCapsule: "run-child-a",
            holdingKind: "write",
          },
        ],
        requestingCapsule: run.id,
        severity: "warning",
      },
    } satisfies RunConflictWarningItem;

    const markup = renderPanel(run, detail, "workspace", [warning]);

    expect(markup).toContain(">Workspace<");
    expect(markup).toContain("/tmp/taugentic-worktrees/run-child-b");
    expect(markup).toContain("ta/capsule-run-child-b");
    expect(markup).toContain("deleteOnSuccess");
    expect(markup).toContain("apps/desktop/package.json");
    expect(markup).toContain("run-child-a");
    expect(markup).toContain("warning");
  });

  it("renders membrane input, workspace, outputs, and replayed timeline events", () => {
    const run = makeRun({
      claimedFiles: ["apps/desktop/package.json"],
      parentRunId: "run-parent",
      recipeId: "patch-agent",
      workspaceInfo: {
        branch: "ta/capsule-run-child-b",
        cleanupPolicy: "deleteOnSuccess",
        path: "/tmp/taugentic-worktrees/run-child-b",
      },
    });
    const detail = makeDetailForRun(run, {
      tokenUsage: {
        promptTokens: 11_000n,
        completionTokens: 1_345n,
        cachedTokens: 2_000n,
        reasoningTokens: 345n,
      },
    });
    const timelineEvents = [
      {
        event: {
          run: {
            detail: "Dispatch workspace prepared",
            outputContract: null,
            recipeId: "patch-agent",
            result: null,
            runId: run.id,
            status: "running",
          },
        },
        seq: 12n,
      },
      {
        event: {
          agentStream: {
            frame: { kind: "toolCallStarted", input: "null", toolName: "shell" },
            fragmentSequence: null,
            itemId: "tool-1",
            runId: run.id,
            turnId: "turn-1",
          },
        },
        seq: 13n,
      },
      {
        event: {
          agentStream: {
            frame: {
              kind: "tokenUsageUpdated",
              modelContextWindow: 200_000n,
              totalTokens: 12_345n,
            },
            fragmentSequence: null,
            itemId: null,
            runId: run.id,
            turnId: "turn-1",
          },
        },
        seq: 14n,
      },
    ] satisfies RunEventDelta[];

    const markup = renderPanel(run, detail, "membrane", [], timelineEvents);

    expect(markup).toContain(">Membrane<");
    expect(markup).toContain("Input Boundary");
    expect(markup).toContain("Workspace Boundary");
    expect(markup).toContain("Outputs");
    expect(markup).toContain("Timeline");
    expect(markup).toContain("patch-agent");
    expect(markup).toContain("/tmp/taugentic-worktrees/run-child-b");
    expect(markup).toContain("12345");
    expect(markup).toContain("11000");
    expect(markup).toContain("1345");
    expect(markup).toContain("2000");
    expect(markup).toContain("345");
    expect(markup).toContain("#12 run running: Dispatch workspace prepared");
    expect(markup).toContain("#13 agent stream toolCallStarted");
    expect(markup).toContain("#14 agent stream tokenUsageUpdated");
  });

  it("renders the typed RunDetail snapshot in the Raw tab", () => {
    const detail = makeDetail();
    const markup = renderPanel(makeRun(), detail, "raw");

    expect(markup).toContain("raw run data");
    expect(markup).toContain("&quot;summary&quot;");
    expect(markup).toContain("run-patch-123456");
    expect(markup).toContain("&quot;runtimeProfileId&quot;");
  });

  it("renders replayed run events in the Logs tab", () => {
    const run = makeRun({ status: "failed" });
    const timelineEvents = [
      {
        event: {
          run: {
            detail: "capsule failed\nbacktrace: frame 1",
            outputContract: null,
            recipeId: null,
            result: null,
            runId: run.id,
            status: "failed",
          },
        },
        seq: 22n,
      },
    ] satisfies RunEventDelta[];

    const markup = renderPanel(
      run,
      makeDetailForRun(run, { result: null }),
      "logs",
      [],
      timelineEvents,
    );

    expect(markup).toContain(">Logs<");
    expect(markup).toContain("#22");
    expect(markup).toContain("capsule failed");
  });

  it("renders accessible tab semantics", () => {
    const markup = renderPanel(makeRun(), makeDetail());

    expect(markup).toContain('role="tablist"');
    expect(markup).toContain('role="tab"');
    expect(markup).toContain('aria-selected="true"');
  });
});
