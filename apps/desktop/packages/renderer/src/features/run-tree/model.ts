import { useCallback, useMemo, useState } from "react";

import { createStore } from "@xstate/store";
import { useSelector } from "@xstate/store/react";

import {
  NATIVE_RUN_LIST_MAX_LIMIT,
  type ListNativeRunsRequest,
  type ListNativeRunsResult,
  type RunListEntry,
  type SessionId,
} from "@taugentic/desktop-shared";

import { useSessionNativeRunsQuery, type SessionQueryView } from "@/lib/queries/session-queries";

import { projectRunTree, type RunTree } from "./projection";

export const DEFAULT_RUN_TREE_NATIVE_RUN_LIMIT = NATIVE_RUN_LIST_MAX_LIMIT;

const DEFAULT_NATIVE_RUNS_REQUEST = {
  limit: DEFAULT_RUN_TREE_NATIVE_RUN_LIMIT,
} satisfies ListNativeRunsRequest;

const EMPTY_RUNS: readonly RunListEntry[] = [];

export type RunTreeExpansionMode = "all" | "custom";

export type RunTreeUiState = {
  selectedRunId: string | null;
  expandedRunIds: Set<string>;
  expansionMode: RunTreeExpansionMode;
};

export type RunTreeStore = ReturnType<typeof createRunTreeStore>;
export type RunTreeSnapshot = ReturnType<RunTreeStore["getSnapshot"]>;

export interface UseRunTreeResult {
  tree: RunTree;
  isLoading: boolean;
  isFetching: boolean;
  error: unknown;
  selectedRunId: string | null;
  expandedRunIds: ReadonlySet<string>;
  select: (id: string) => void;
  toggleExpand: (id: string) => void;
  expandAll: () => void;
  collapseAll: () => void;
  refetch: SessionQueryView<ListNativeRunsResult>["refetch"];
}

function createInitialRunTreeUiState(): RunTreeUiState {
  return {
    selectedRunId: null,
    expandedRunIds: new Set<string>(),
    expansionMode: "all",
  };
}

export function createRunTreeStore() {
  return createStore({
    context: createInitialRunTreeUiState(),
    on: {
      allCollapsed: (context) => ({
        ...context,
        expandedRunIds: new Set<string>(),
        expansionMode: "custom" as const,
      }),
      allExpanded: (context) => ({
        ...context,
        expandedRunIds: new Set<string>(),
        expansionMode: "all" as const,
      }),
      runExpansionToggled: (
        context,
        event: {
          defaultExpandedRunIds: ReadonlySet<string>;
          runId: string;
        },
      ) => {
        const expandedRunIds = new Set(
          context.expansionMode === "all" ? event.defaultExpandedRunIds : context.expandedRunIds,
        );

        if (expandedRunIds.has(event.runId)) {
          expandedRunIds.delete(event.runId);
        } else {
          expandedRunIds.add(event.runId);
        }

        return {
          ...context,
          expandedRunIds,
          expansionMode: "custom" as const,
        };
      },
      runSelected: (
        context,
        event: {
          runId: string | null;
        },
      ) => ({
        ...context,
        selectedRunId: event.runId,
      }),
    },
  });
}

function selectRunTreeUiState(snapshot: RunTreeSnapshot): RunTreeUiState {
  return snapshot.context;
}

export function selectRun(store: RunTreeStore, runId: string): void {
  store.trigger.runSelected({ runId });
}

export function toggleRunTreeExpansion(
  store: RunTreeStore,
  runId: string,
  defaultExpandedRunIds: ReadonlySet<string>,
): void {
  store.trigger.runExpansionToggled({ defaultExpandedRunIds, runId });
}

export function expandAllRunTreeNodes(store: RunTreeStore): void {
  store.trigger.allExpanded();
}

export function collapseAllRunTreeNodes(store: RunTreeStore): void {
  store.trigger.allCollapsed();
}

export function useRunTree(
  sessionId: SessionId | null,
  request: ListNativeRunsRequest = DEFAULT_NATIVE_RUNS_REQUEST,
): UseRunTreeResult {
  const [store] = useState(() => createRunTreeStore());
  const query = useSessionNativeRunsQuery(sessionId, request);
  const runs = query.data?.runs ?? EMPTY_RUNS;
  const tree = useMemo(() => projectRunTree(runs), [runs]);
  const uiState = useSelector(store, selectRunTreeUiState);
  const defaultExpandedRunIds = useMemo(() => collectParentRunIds(tree), [tree]);
  const expandedRunIds = useMemo(
    () =>
      uiState.expansionMode === "all" ? defaultExpandedRunIds : new Set(uiState.expandedRunIds),
    [defaultExpandedRunIds, uiState.expandedRunIds, uiState.expansionMode],
  );

  const select = useCallback((id: string) => selectRun(store, id), [store]);
  const toggleExpand = useCallback(
    (id: string) => toggleRunTreeExpansion(store, id, defaultExpandedRunIds),
    [defaultExpandedRunIds, store],
  );
  const expandAll = useCallback(() => expandAllRunTreeNodes(store), [store]);
  const collapseAll = useCallback(() => collapseAllRunTreeNodes(store), [store]);

  return {
    tree,
    isLoading: query.isLoading,
    isFetching: query.isFetching,
    error: query.error,
    selectedRunId: uiState.selectedRunId,
    expandedRunIds,
    select,
    toggleExpand,
    expandAll,
    collapseAll,
    refetch: query.refetch,
  };
}

function collectParentRunIds(tree: RunTree): ReadonlySet<string> {
  const parentRunIds = new Set<string>();
  for (const [runId, node] of tree.byId) {
    if (node.children.length > 0) {
      parentRunIds.add(runId);
    }
  }
  return parentRunIds;
}
