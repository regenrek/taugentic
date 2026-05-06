import type { JSX } from "react";

import type { RunListEntry, SessionId } from "@taugentic/desktop-shared";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/ui/cn";

import { SectionFeedback } from "../session-detail/section-feedback";
import { SectionHeader } from "../session-detail/section-header";
import { RunDetailPanel, RunDetailPanelProvider } from "./RunDetailPanel";
import { useRunTree, type UseRunTreeResult } from "./model";
import type { RunTree, RunTreeNode } from "./projection";
import { RunTreeNodeView } from "./RunTreeNodeView";

export interface RunTreeSectionProps {
  className?: string;
  sessionId?: SessionId | null;
}

export function RunTreeSection({
  className,
  sessionId = null,
}: RunTreeSectionProps = {}): JSX.Element {
  const runTree = useRunTree(sessionId);
  return <RunTreeSectionView className={className} runTree={runTree} sessionId={sessionId} />;
}

export interface RunTreeSectionViewProps {
  className?: string;
  runTree: UseRunTreeResult;
  sessionId?: SessionId | null;
}

export function RunTreeSectionView({
  className,
  runTree,
  sessionId = null,
}: RunTreeSectionViewProps): JSX.Element {
  const errorMessage = runTree.error ? toErrorMessage(runTree.error) : null;
  const hasLoaded = !runTree.isLoading;
  const runCount = runTree.tree.byId.size;
  const hasRuns = runCount > 0;
  const visibleRunIds = collectVisibleRunIds(runTree.tree, runTree.expandedRunIds);
  const focusedRunId = runTree.selectedRunId ?? visibleRunIds[0] ?? null;
  const selectedRun = findSelectedRun(runTree.tree, runTree.selectedRunId);

  function moveFocus(runId: string, direction: "next" | "previous") {
    const currentIndex = visibleRunIds.indexOf(runId);
    if (currentIndex === -1) {
      return;
    }

    const nextIndex = direction === "next" ? currentIndex + 1 : currentIndex - 1;
    const nextRunId = visibleRunIds[nextIndex];
    if (nextRunId === undefined) {
      return;
    }

    runTree.select(nextRunId);
    focusRunTreeItem(nextRunId);
  }

  return (
    <RunDetailPanelProvider
      run={selectedRun}
      selectedRunId={runTree.selectedRunId}
      sessionId={sessionId}
    >
      <section className={cn("flex flex-col gap-2 px-3 py-3", className)} data-section="run-tree">
        <SectionHeader
          count={runCount}
          errorMessage={errorMessage}
          hasLoaded={hasLoaded}
          label="run tree"
          pending={runTree.isFetching || runTree.isLoading}
          trailing={
            hasRuns ? (
              <div className="flex items-center gap-1">
                <Button
                  className="h-6 px-1.5 text-[9px] tracking-[0.14em]"
                  onClick={runTree.expandAll}
                  size="sm"
                  type="button"
                  variant="ghost"
                >
                  expand
                </Button>
                <Button
                  className="h-6 px-1.5 text-[9px] tracking-[0.14em]"
                  onClick={runTree.collapseAll}
                  size="sm"
                  type="button"
                  variant="ghost"
                >
                  collapse
                </Button>
              </div>
            ) : undefined
          }
        />
        <SectionFeedback
          errorMessage={errorMessage}
          hasLoaded={hasLoaded}
          isEmpty={!hasRuns}
          itemsLabel="native runs"
        />
        <div className="flex min-h-0 flex-col gap-3 xl:flex-row">
          <div className="min-w-0 flex-1">
            {runTree.tree.roots.length > 0 ? (
              <RunTreeView
                expandedRunIds={runTree.expandedRunIds}
                focusedRunId={focusedRunId}
                label="Native run hierarchy"
                onMoveFocus={moveFocus}
                onSelect={runTree.select}
                onToggleExpand={runTree.toggleExpand}
                selectedRunId={runTree.selectedRunId}
                tree={runTree.tree}
              />
            ) : null}
            {runTree.tree.orphans.length > 0 ? (
              <OrphanRunsSection
                expandedRunIds={runTree.expandedRunIds}
                focusedRunId={focusedRunId}
                onMoveFocus={moveFocus}
                onSelect={runTree.select}
                onToggleExpand={runTree.toggleExpand}
                orphans={runTree.tree.orphans}
                selectedRunId={runTree.selectedRunId}
              />
            ) : null}
          </div>
          <RunDetailPanel />
        </div>
      </section>
    </RunDetailPanelProvider>
  );
}

interface RunTreeViewProps {
  expandedRunIds: ReadonlySet<string>;
  focusedRunId: string | null;
  label: string;
  onMoveFocus: (runId: string, direction: "next" | "previous") => void;
  onSelect: (runId: string) => void;
  onToggleExpand: (runId: string) => void;
  selectedRunId: string | null;
  tree: Pick<RunTree, "roots">;
}

function RunTreeView({
  expandedRunIds,
  focusedRunId,
  label,
  onMoveFocus,
  onSelect,
  onToggleExpand,
  selectedRunId,
  tree,
}: RunTreeViewProps): JSX.Element {
  return (
    <div aria-label={label} className="flex flex-col gap-0.5" data-run-tree-root="" role="tree">
      {tree.roots.map((node) => (
        <RunTreeNodeView
          key={node.run.id}
          expandedRunIds={expandedRunIds}
          focusedRunId={focusedRunId}
          node={node}
          onMoveFocus={onMoveFocus}
          onSelect={onSelect}
          onToggleExpand={onToggleExpand}
          selectedRunId={selectedRunId}
        />
      ))}
    </div>
  );
}

interface OrphanRunsSectionProps {
  expandedRunIds: ReadonlySet<string>;
  focusedRunId: string | null;
  onMoveFocus: (runId: string, direction: "next" | "previous") => void;
  onSelect: (runId: string) => void;
  onToggleExpand: (runId: string) => void;
  orphans: ReadonlyArray<RunListEntry>;
  selectedRunId: string | null;
}

function OrphanRunsSection({
  expandedRunIds,
  focusedRunId,
  onMoveFocus,
  onSelect,
  onToggleExpand,
  orphans,
  selectedRunId,
}: OrphanRunsSectionProps): JSX.Element {
  return (
    <div className="mt-1 border-t border-[var(--border)]/60 pt-2" data-run-tree-orphans="">
      <div className="mb-1 font-[var(--font-mono)] text-[10px] uppercase tracking-[0.18em] text-[var(--fg-dim)]">
        Orphan runs
      </div>
      <div aria-label="Orphan runs" className="flex flex-col gap-0.5" role="tree">
        {orphans.map((run) => (
          <RunTreeNodeView
            key={run.id}
            expandedRunIds={expandedRunIds}
            focusedRunId={focusedRunId}
            node={createOrphanNode(run)}
            onMoveFocus={onMoveFocus}
            onSelect={onSelect}
            onToggleExpand={onToggleExpand}
            selectedRunId={selectedRunId}
          />
        ))}
      </div>
    </div>
  );
}

function createOrphanNode(run: RunListEntry): RunTreeNode {
  return {
    children: [],
    depth: 0,
    run,
  };
}

function collectVisibleRunIds(tree: RunTree, expandedRunIds: ReadonlySet<string>): string[] {
  const visibleRunIds: string[] = [];

  for (const root of tree.roots) {
    collectVisibleNodeIds(root, expandedRunIds, visibleRunIds);
  }
  for (const orphan of tree.orphans) {
    visibleRunIds.push(orphan.id);
  }

  return visibleRunIds;
}

function findSelectedRun(tree: RunTree, selectedRunId: string | null): RunListEntry | null {
  if (selectedRunId === null) {
    return null;
  }

  return (
    tree.byId.get(selectedRunId)?.run ??
    tree.orphans.find((run) => run.id === selectedRunId) ??
    null
  );
}

function collectVisibleNodeIds(
  node: RunTreeNode,
  expandedRunIds: ReadonlySet<string>,
  visibleRunIds: string[],
): void {
  visibleRunIds.push(node.run.id);

  if (!expandedRunIds.has(node.run.id)) {
    return;
  }

  for (const child of node.children) {
    collectVisibleNodeIds(child, expandedRunIds, visibleRunIds);
  }
}

function focusRunTreeItem(runId: string): void {
  if (typeof document === "undefined") {
    return;
  }

  for (const element of document.querySelectorAll<HTMLElement>("[data-run-tree-focus-id]")) {
    if (element.dataset.runTreeFocusId === runId) {
      element.focus();
      return;
    }
  }
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
