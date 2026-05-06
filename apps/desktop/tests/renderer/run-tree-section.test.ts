import { describe, expect, it, vi } from "vite-plus/test";

import type { RunListEntry } from "../../packages/shared/src/contracts.js";
import {
  projectRunTree,
  RunTreeNodeView,
  RunTreeSectionView,
  RunTreeStatusBadge,
  type UseRunTreeResult,
} from "../../packages/renderer/src/features/run-tree/index.js";

type CreateElementFn = (
  component: unknown,
  props?: Record<string, unknown> | null,
  ...children: unknown[]
) => unknown;
type RenderToStaticMarkupFn = (element: unknown) => string;
type ElementLike = {
  props?: Record<string, unknown>;
  type?: unknown;
};

const reactModulePath = "../../packages/renderer/node_modules/react/index.js";
const reactServerModulePath = "../../packages/renderer/node_modules/react-dom/server.node.js";

const { createElement } = (await import(reactModulePath)) as {
  createElement: CreateElementFn;
};
const { renderToStaticMarkup } = (await import(reactServerModulePath)) as {
  renderToStaticMarkup: RenderToStaticMarkupFn;
};

function makeRun(id: string, overrides: Partial<RunListEntry> = {}): RunListEntry {
  return {
    id,
    harness: "native",
    status: "running",
    ...overrides,
  };
}

function makeRunTreeResult(
  runs: RunListEntry[],
  overrides: Partial<UseRunTreeResult> = {},
): UseRunTreeResult {
  const tree = projectRunTree(runs);
  return {
    tree,
    isLoading: false,
    isFetching: false,
    error: null,
    selectedRunId: null,
    expandedRunIds: new Set(
      [...tree.byId.values()].filter((node) => node.children.length > 0).map((node) => node.run.id),
    ),
    select: vi.fn(),
    toggleExpand: vi.fn(),
    expandAll: vi.fn(),
    collapseAll: vi.fn(),
    refetch: vi.fn(),
    ...overrides,
  };
}

function renderRunTree(result: UseRunTreeResult): string {
  return renderToStaticMarkup(createElement(RunTreeSectionView, { runTree: result }));
}

describe("RunTreeSection", () => {
  it("renders the empty state when no native runs exist", () => {
    const markup = renderRunTree(makeRunTreeResult([]));

    expect(markup).toContain('data-section="run-tree"');
    expect(markup).toContain('data-state="empty"');
    expect(markup).toContain("no native runs yet");
  });

  it("renders a single root with the mapped status dot", () => {
    const markup = renderRunTree(
      makeRunTreeResult([
        makeRun("run-root", {
          objectivePreview: "ship run tree",
          status: "completed",
        }),
      ]),
    );

    expect(markup).toContain("ship run tree");
    expect(markup).toContain('role="treeitem"');
    expect(markup).toContain('data-tone="completed"');
  });

  it("renders an expanded child under its parent with depth indentation", () => {
    const markup = renderRunTree(
      makeRunTreeResult([
        makeRun("run-parent", { objectivePreview: "parent" }),
        makeRun("run-child", {
          objectivePreview: "child",
          parentRunId: "run-parent",
        }),
      ]),
    );

    expect(markup.indexOf("parent")).toBeLessThan(markup.indexOf("child"));
    expect(markup).toContain('aria-expanded="true"');
    expect(markup).toContain("padding-left:16px");
    expect(markup).toContain('data-section-hint="count">2');
  });

  it("does not render children when the parent is collapsed", () => {
    const markup = renderRunTree(
      makeRunTreeResult(
        [
          makeRun("run-parent", { objectivePreview: "parent" }),
          makeRun("run-child", {
            objectivePreview: "child",
            parentRunId: "run-parent",
          }),
        ],
        { expandedRunIds: new Set() },
      ),
    );

    expect(markup).toContain("parent");
    expect(markup).not.toContain("child");
    expect(markup).toContain('aria-expanded="false"');
  });

  it("renders the status badge matrix with canonical labels and variants", () => {
    const cases = [
      ["running", "running", "secondary"],
      ["completed", "completed", "accent"],
      ["failed", "failed", "destructive"],
      ["contractViolation", "contract violation", "destructive"],
      ["quarantined", "quarantined", "destructive"],
    ] as const;

    for (const [status, label, variant] of cases) {
      const markup = renderToStaticMarkup(createElement(RunTreeStatusBadge, { status }));

      expect(markup).toContain(label);
      expect(markup).toContain(`data-variant="${variant}"`);
    }
  });

  it("renders recipeId as a compact tag", () => {
    const markup = renderRunTree(
      makeRunTreeResult([
        makeRun("run-recipe", {
          recipeId: "debug-agent",
        }),
      ]),
    );

    expect(markup).toContain("[debug-agent]");
    expect(markup).toContain('title="debug-agent"');
  });

  it("renders a conflict badge from the canonical run summary", () => {
    const markup = renderRunTree(
      makeRunTreeResult([
        makeRun("run-conflict", {
          conflictSummary: {
            files: ["apps/desktop/package.json"],
            warningCount: 2,
          },
          objectivePreview: "conflicting capsule",
        }),
      ]),
    );

    expect(markup).toContain("2 conflicts");
    expect(markup).toContain('title="Conflict files: apps/desktop/package.json"');
    expect(markup).toContain('data-variant="destructive"');
  });

  it("selects a run through the node row action", () => {
    const tree = projectRunTree([makeRun("run-select")]);
    const node = tree.roots[0];
    expect(node).toBeDefined();

    let selectedRunId: string | null = null;
    const element = RunTreeNodeView({
      expandedRunIds: new Set(),
      focusedRunId: "run-select",
      node: node!,
      onMoveFocus: vi.fn(),
      onSelect: (runId) => {
        selectedRunId = runId;
      },
      onToggleExpand: vi.fn(),
      selectedRunId,
    });
    const selectAction = findElementByDataAttribute(element, "data-run-tree-node-action", "select");

    expect(selectAction).not.toBeNull();
    if (selectAction === null) {
      throw new Error("expected select action");
    }
    const onSelectClick = selectAction.props?.onClick;
    if (typeof onSelectClick !== "function") {
      throw new Error("expected select action click handler");
    }
    onSelectClick();
    expect(selectedRunId).toBe("run-select");

    const selectedMarkup = renderToStaticMarkup(
      createElement(RunTreeNodeView, {
        expandedRunIds: new Set(),
        focusedRunId: "run-select",
        node,
        onMoveFocus: vi.fn(),
        onSelect: vi.fn(),
        onToggleExpand: vi.fn(),
        selectedRunId,
      }),
    );
    expect(selectedMarkup).toContain('aria-selected="true"');
  });

  it("renders orphan runs in a separate section", () => {
    const markup = renderRunTree(
      makeRunTreeResult([
        makeRun("run-orphan", {
          parentRunId: "missing-parent",
        }),
      ]),
    );

    expect(markup).toContain("Orphan runs");
    expect(markup).toContain('data-run-tree-orphans=""');
    expect(markup).toContain("run-orphan");
  });
});

function findElementByDataAttribute(
  value: unknown,
  attribute: string,
  expected: string,
): ElementLike | null {
  if (!isElementLike(value)) {
    return null;
  }

  if (value.props?.[attribute] === expected) {
    return value;
  }

  for (const child of childrenFrom(value.props?.children)) {
    const match = findElementByDataAttribute(child, attribute, expected);
    if (match !== null) {
      return match;
    }
  }

  return null;
}

function isElementLike(value: unknown): value is ElementLike {
  return typeof value === "object" && value !== null && "props" in value;
}

function childrenFrom(children: unknown): unknown[] {
  if (children === null || children === undefined || typeof children === "boolean") {
    return [];
  }
  return Array.isArray(children) ? children : [children];
}
