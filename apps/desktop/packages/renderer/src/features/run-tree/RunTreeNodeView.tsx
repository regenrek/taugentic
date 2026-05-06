import type { CSSProperties, KeyboardEvent, MouseEvent } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Collapsible } from "@/components/ui/collapsible";
import { StatusDot } from "@/components/ui/status-dot";
import {
  getRunStatusPresentation,
  type RunPresentationStatus,
} from "@/features/run-status/presentation";
import { cn } from "@/lib/ui/cn";

import type { RunTreeNode } from "./projection";

const RUN_TREE_INDENT_PX = 16;

export interface RunTreeNodeViewProps {
  expandedRunIds: ReadonlySet<string>;
  focusedRunId: string | null;
  node: RunTreeNode;
  onMoveFocus: (runId: string, direction: "next" | "previous") => void;
  onSelect: (runId: string) => void;
  onToggleExpand: (runId: string) => void;
  selectedRunId: string | null;
}

export function RunTreeNodeView({
  expandedRunIds,
  focusedRunId,
  node,
  onMoveFocus,
  onSelect,
  onToggleExpand,
  selectedRunId,
}: RunTreeNodeViewProps) {
  const { run } = node;
  const hasChildren = node.children.length > 0;
  const isExpanded = hasChildren && expandedRunIds.has(run.id);
  const isSelected = selectedRunId === run.id;
  const status = getRunStatusPresentation(run.status as RunPresentationStatus);
  const elapsed = formatElapsed(run.startedAtMs, run.endedAtMs);
  const rowStyle: CSSProperties = {
    paddingLeft: node.depth * RUN_TREE_INDENT_PX,
  };

  function selectRun() {
    onSelect(run.id);
  }

  function toggleExpansion(event: MouseEvent<HTMLButtonElement>) {
    event.stopPropagation();
    onToggleExpand(run.id);
  }

  function handleTreeItemKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    switch (event.key) {
      case "Enter":
      case " ":
        event.preventDefault();
        selectRun();
        return;
      case "ArrowDown":
        event.preventDefault();
        onMoveFocus(run.id, "next");
        return;
      case "ArrowUp":
        event.preventDefault();
        onMoveFocus(run.id, "previous");
        return;
      case "ArrowRight":
        if (hasChildren && !isExpanded) {
          event.preventDefault();
          onToggleExpand(run.id);
        }
        return;
      case "ArrowLeft":
        if (hasChildren && isExpanded) {
          event.preventDefault();
          onToggleExpand(run.id);
        }
        return;
    }
  }

  return (
    <Collapsible.Root open={isExpanded}>
      <div
        aria-expanded={hasChildren ? isExpanded : undefined}
        aria-level={node.depth + 1}
        aria-selected={isSelected}
        className={cn(
          "group flex items-stretch gap-1 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--ring,var(--fg))]",
          isSelected ? "bg-[var(--bg-raised)]" : undefined,
        )}
        data-run-tree-focus-id={run.id}
        data-run-tree-node={run.id}
        data-run-tree-node-action="select"
        onClick={selectRun}
        onKeyDown={handleTreeItemKeyDown}
        role="treeitem"
        tabIndex={focusedRunId === run.id ? 0 : -1}
      >
        <div className="flex min-w-0 flex-1 items-stretch gap-1" style={rowStyle}>
          {hasChildren ? (
            <Collapsible.Trigger
              aria-label={`${isExpanded ? "Collapse" : "Expand"} ${run.id}`}
              className="h-7 w-5 shrink-0 justify-center px-0 text-[var(--fg-mute)] hover:text-[var(--fg)]"
              data-run-tree-node-action="toggle"
              onClick={toggleExpansion}
              tabIndex={-1}
              type="button"
            >
              {isExpanded ? (
                <ChevronDown aria-hidden="true" className="size-3" />
              ) : (
                <ChevronRight aria-hidden="true" className="size-3" />
              )}
            </Collapsible.Trigger>
          ) : (
            <span aria-hidden="true" className="h-7 w-5 shrink-0" />
          )}
          <div
            className={cn(
              "flex min-w-0 flex-1 items-center gap-2 rounded-[var(--radius)] border border-transparent px-1.5 py-1 text-left font-[var(--font-mono)] text-[12px] text-[var(--fg)] transition-colors",
              "hover:border-[var(--border)] hover:bg-[var(--bg-hover,var(--border))]",
              isSelected ? "border-[var(--accent,var(--border))] bg-[var(--bg-raised)]" : undefined,
            )}
          >
            <StatusDot className="shrink-0" tone={status.tone} />
            <span className="min-w-0 flex-1 truncate" title={run.objectivePreview ?? run.id}>
              {formatRunTitle(run.objectivePreview, run.id)}
            </span>
            {run.recipeId ? (
              <Badge
                className="max-w-[11rem] shrink truncate border-[var(--border)]/70 bg-[var(--bg-subtle,transparent)] px-1.5 py-0 text-[9px] normal-case tracking-normal text-[var(--fg-mute)]"
                title={run.recipeId}
                variant="outline"
              >
                [{run.recipeId}]
              </Badge>
            ) : null}
            {run.conflictSummary && run.conflictSummary.warningCount > 0 ? (
              <Badge
                className="shrink-0 px-1.5 py-0 text-[9px]"
                title={formatConflictSummaryTitle(run.conflictSummary.files)}
                variant="destructive"
              >
                {run.conflictSummary.warningCount === 1
                  ? "1 conflict"
                  : `${run.conflictSummary.warningCount} conflicts`}
              </Badge>
            ) : null}
            <RunTreeStatusBadge status={run.status as RunPresentationStatus} />
            {elapsed !== null ? (
              <span className="shrink-0 text-[10px] uppercase tracking-[0.14em] text-[var(--fg-dim)]">
                {elapsed}
              </span>
            ) : null}
          </div>
        </div>
      </div>
      {hasChildren && isExpanded ? (
        <Collapsible.Content>
          <div role="group">
            {node.children.map((child) => (
              <RunTreeNodeView
                key={child.run.id}
                expandedRunIds={expandedRunIds}
                focusedRunId={focusedRunId}
                node={child}
                onMoveFocus={onMoveFocus}
                onSelect={onSelect}
                onToggleExpand={onToggleExpand}
                selectedRunId={selectedRunId}
              />
            ))}
          </div>
        </Collapsible.Content>
      ) : null}
    </Collapsible.Root>
  );
}

export function RunTreeStatusBadge({ status }: { status: RunPresentationStatus }) {
  const presentation = getRunStatusPresentation(status);
  return (
    <Badge className="shrink-0 px-1.5 py-0 text-[9px]" variant={presentation.badgeVariant}>
      {presentation.label}
    </Badge>
  );
}

function formatConflictSummaryTitle(files: string[]): string {
  if (files.length === 0) {
    return "Conflict warning";
  }
  return `Conflict files: ${files.join(", ")}`;
}

function formatRunTitle(objectivePreview: string | null | undefined, runId: string): string {
  const title = objectivePreview?.trim();
  if (title && title.length > 0) {
    return title;
  }
  return shortRunId(runId);
}

function shortRunId(runId: string): string {
  if (runId.length <= 12) {
    return runId;
  }
  return `${runId.slice(0, 8)}...${runId.slice(-4)}`;
}

function formatElapsed(
  startedAtMs: bigint | number | null | undefined,
  endedAtMs: bigint | number | null | undefined,
): string | null {
  if (startedAtMs === null || startedAtMs === undefined) {
    return null;
  }

  const started = toNumber(startedAtMs);
  const ended = endedAtMs === null || endedAtMs === undefined ? Date.now() : toNumber(endedAtMs);
  const elapsedSeconds = Math.max(0, Math.floor((ended - started) / 1_000));

  if (elapsedSeconds < 60) {
    return `${elapsedSeconds}s`;
  }

  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60) {
    return `${elapsedMinutes}m`;
  }

  return `${Math.floor(elapsedMinutes / 60)}h`;
}

function toNumber(value: bigint | number): number {
  return typeof value === "bigint" ? Number(value) : value;
}
