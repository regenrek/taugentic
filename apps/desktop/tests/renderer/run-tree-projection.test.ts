import { describe, expect, it, vi } from "vite-plus/test";

import {
  NATIVE_RUN_LIST_MAX_LIMIT,
  type RunListEntry,
} from "../../packages/shared/src/contracts.js";
import {
  collapseAllRunTreeNodes,
  createRunTreeStore,
  DEFAULT_RUN_TREE_NATIVE_RUN_LIMIT,
  expandAllRunTreeNodes,
  projectRunTree,
  selectRun,
  toggleRunTreeExpansion,
  useRunTree,
} from "../../packages/renderer/src/features/run-tree/index.js";

function makeRun(id: string, overrides: Partial<RunListEntry> = {}): RunListEntry {
  return {
    id,
    harness: "native",
    status: "running",
    ...overrides,
  };
}

function ids(runs: ReadonlyArray<RunListEntry>): string[] {
  return runs.map((run) => run.id);
}

describe("run tree projection", () => {
  it("projects empty input to an empty tree", () => {
    const tree = projectRunTree([]);

    expect(tree.roots).toEqual([]);
    expect(tree.orphans).toEqual([]);
    expect(tree.byId.size).toBe(0);
  });

  it("projects a single top-level run as one depth-0 root", () => {
    const tree = projectRunTree([makeRun("root")]);

    expect(tree.roots).toHaveLength(1);
    expect(tree.roots[0]?.run.id).toBe("root");
    expect(tree.roots[0]?.depth).toBe(0);
    expect(tree.roots[0]?.children).toEqual([]);
  });

  it("attaches a child run to its parent", () => {
    const tree = projectRunTree([makeRun("child", { parentRunId: "root" }), makeRun("root")]);

    expect(tree.roots).toHaveLength(1);
    expect(tree.roots[0]?.children).toHaveLength(1);
    expect(tree.roots[0]?.children[0]?.run.id).toBe("child");
    expect(tree.roots[0]?.children[0]?.depth).toBe(1);
  });

  it("assigns depths through deep nesting", () => {
    const tree = projectRunTree([
      makeRun("grandchild", { parentRunId: "child" }),
      makeRun("root"),
      makeRun("child", { parentRunId: "root" }),
    ]);

    const child = tree.byId.get("child");
    const grandchild = tree.byId.get("grandchild");

    expect(tree.roots[0]?.depth).toBe(0);
    expect(child?.depth).toBe(1);
    expect(grandchild?.depth).toBe(2);
  });

  it("sorts siblings by started time and then id", () => {
    const tree = projectRunTree([
      makeRun("root"),
      makeRun("child-c", { parentRunId: "root", startedAtMs: 30n }),
      makeRun("child-a", { parentRunId: "root", startedAtMs: 10n }),
      makeRun("child-b", { parentRunId: "root", startedAtMs: 20n }),
    ]);

    expect(tree.roots[0]?.children.map((node) => node.run.id)).toEqual([
      "child-a",
      "child-b",
      "child-c",
    ]);
  });

  it("keeps multiple top-level runs as separate roots", () => {
    const tree = projectRunTree([
      makeRun("root-b", { startedAtMs: 20n }),
      makeRun("root-a", { startedAtMs: 10n }),
    ]);

    expect(tree.roots.map((node) => node.run.id)).toEqual(["root-a", "root-b"]);
  });

  it("keeps runs with missing parents in the orphan list", () => {
    const tree = projectRunTree([makeRun("child", { parentRunId: "missing-root" })]);

    expect(tree.roots).toEqual([]);
    expect(ids(tree.orphans)).toEqual(["child"]);
  });

  it("treats cyclic parent chains as orphans", () => {
    const logger = {
      warn: vi.fn(),
    };
    const tree = projectRunTree(
      [makeRun("a", { parentRunId: "b" }), makeRun("b", { parentRunId: "a" })],
      { logger },
    );

    expect(tree.roots).toEqual([]);
    expect(ids(tree.orphans)).toEqual(["a", "b"]);
    expect(logger.warn).toHaveBeenCalledWith(
      "Ignoring cyclic run tree parent links for run ids: a, b",
    );
  });

  it("populates the by-id lookup for roots, children, and orphans", () => {
    const tree = projectRunTree([
      makeRun("root"),
      makeRun("child", { parentRunId: "root" }),
      makeRun("orphan", { parentRunId: "missing-root" }),
    ]);

    expect(tree.byId.get("root")?.run.id).toBe("root");
    expect(tree.byId.get("child")?.run.id).toBe("child");
    expect(tree.byId.get("orphan")?.run.id).toBe("orphan");
  });
});

describe("run tree UI state store", () => {
  it("exports the composition hook for UI integration", () => {
    expect(useRunTree).toBeTypeOf("function");
  });

  it("uses the generated wire max limit for default native run queries", () => {
    expect(DEFAULT_RUN_TREE_NATIVE_RUN_LIMIT).toBe(NATIVE_RUN_LIST_MAX_LIMIT);
  });

  it("selects runs and toggles expansion without mirroring run data", () => {
    const store = createRunTreeStore();
    const defaultExpandedRunIds = new Set(["parent"]);

    expect(store.getSnapshot().context).toMatchObject({
      selectedRunId: null,
      expansionMode: "all",
    });

    selectRun(store, "child");
    expect(store.getSnapshot().context.selectedRunId).toBe("child");

    toggleRunTreeExpansion(store, "parent", defaultExpandedRunIds);
    expect(store.getSnapshot().context.expansionMode).toBe("custom");
    expect([...store.getSnapshot().context.expandedRunIds]).toEqual([]);

    toggleRunTreeExpansion(store, "parent", defaultExpandedRunIds);
    expect([...store.getSnapshot().context.expandedRunIds]).toEqual(["parent"]);

    expandAllRunTreeNodes(store);
    expect(store.getSnapshot().context.expansionMode).toBe("all");
    expect([...store.getSnapshot().context.expandedRunIds]).toEqual([]);

    collapseAllRunTreeNodes(store);
    expect(store.getSnapshot().context.expansionMode).toBe("custom");
    expect([...store.getSnapshot().context.expandedRunIds]).toEqual([]);
  });
});
