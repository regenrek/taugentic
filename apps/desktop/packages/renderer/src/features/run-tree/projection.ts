import type { RunListEntry } from "@taugentic/desktop-shared";

export type RunTreeNode = {
  run: RunListEntry;
  children: RunTreeNode[];
  depth: number;
};

export type RunTree = {
  roots: RunTreeNode[];
  byId: Map<string, RunTreeNode>;
  orphans: RunListEntry[];
};

export interface RunTreeLogger {
  warn(message: string): void;
}

export interface ProjectRunTreeOptions {
  logger?: RunTreeLogger;
}

type VisitState = "done" | "visiting";

export function projectRunTree(
  runs: ReadonlyArray<RunListEntry>,
  options: ProjectRunTreeOptions = {},
): RunTree {
  const byId = new Map<string, RunTreeNode>();
  const parentById = new Map<string, string | null>();

  for (const run of runs) {
    byId.set(run.id, {
      run,
      children: [],
      depth: 0,
    });
    parentById.set(run.id, run.parentRunId ?? null);
  }

  const cycleIds = findCycleIds(byId, parentById);
  if (cycleIds.size > 0) {
    options.logger?.warn(
      `Ignoring cyclic run tree parent links for run ids: ${[...cycleIds].sort().join(", ")}`,
    );
  }

  const roots: RunTreeNode[] = [];
  const orphans: RunListEntry[] = [];
  const treeReachableById = createTreeReachabilityIndex(byId, parentById, cycleIds);

  for (const node of byId.values()) {
    if (!treeReachableById.get(node.run.id)) {
      orphans.push(node.run);
      continue;
    }

    const parentRunId = parentById.get(node.run.id) ?? null;
    if (parentRunId === null) {
      roots.push(node);
      continue;
    }

    byId.get(parentRunId)?.children.push(node);
  }

  for (const node of byId.values()) {
    node.children.sort(compareRunTreeNodes);
  }
  roots.sort(compareRunTreeNodes);
  orphans.sort(compareRunListEntries);
  assignDepths(roots);

  return {
    roots,
    byId,
    orphans,
  };
}

function findCycleIds(
  byId: ReadonlyMap<string, RunTreeNode>,
  parentById: ReadonlyMap<string, string | null>,
): Set<string> {
  const visitStateById = new Map<string, VisitState>();
  const cycleIds = new Set<string>();

  for (const id of byId.keys()) {
    if (visitStateById.has(id)) {
      continue;
    }

    const path: string[] = [];
    const pathIndexById = new Map<string, number>();
    let currentId: string | null = id;

    while (currentId !== null && byId.has(currentId)) {
      const visitState = visitStateById.get(currentId);
      if (visitState === "done") {
        break;
      }

      if (visitState === "visiting") {
        const cycleStartIndex = pathIndexById.get(currentId);
        if (cycleStartIndex !== undefined) {
          for (const cycleId of path.slice(cycleStartIndex)) {
            cycleIds.add(cycleId);
          }
        }
        break;
      }

      visitStateById.set(currentId, "visiting");
      pathIndexById.set(currentId, path.length);
      path.push(currentId);
      currentId = parentById.get(currentId) ?? null;
    }

    for (const pathId of path) {
      visitStateById.set(pathId, "done");
    }
  }

  return cycleIds;
}

function createTreeReachabilityIndex(
  byId: ReadonlyMap<string, RunTreeNode>,
  parentById: ReadonlyMap<string, string | null>,
  cycleIds: ReadonlySet<string>,
): Map<string, boolean> {
  const treeReachableById = new Map<string, boolean>();

  for (const id of byId.keys()) {
    if (treeReachableById.has(id)) {
      continue;
    }

    const path: string[] = [];
    let currentId: string | null = id;
    let isTreeReachable = false;

    while (currentId !== null) {
      const knownReachability = treeReachableById.get(currentId);
      if (knownReachability !== undefined) {
        isTreeReachable = knownReachability;
        break;
      }

      if (cycleIds.has(currentId)) {
        isTreeReachable = false;
        break;
      }

      path.push(currentId);
      const parentRunId: string | null = parentById.get(currentId) ?? null;
      if (parentRunId === null) {
        isTreeReachable = true;
        break;
      }

      if (!byId.has(parentRunId) || cycleIds.has(parentRunId)) {
        isTreeReachable = false;
        break;
      }

      currentId = parentRunId;
    }

    for (const pathId of path) {
      treeReachableById.set(pathId, isTreeReachable);
    }
  }

  return treeReachableById;
}

function assignDepths(roots: RunTreeNode[]): void {
  const pending = roots.map((node) => ({ depth: 0, node }));

  while (pending.length > 0) {
    const item = pending.pop();
    if (!item) {
      continue;
    }

    item.node.depth = item.depth;
    for (let index = item.node.children.length - 1; index >= 0; index -= 1) {
      pending.push({
        depth: item.depth + 1,
        node: item.node.children[index],
      });
    }
  }
}

function compareRunTreeNodes(left: RunTreeNode, right: RunTreeNode): number {
  return compareRunListEntries(left.run, right.run);
}

function compareRunListEntries(left: RunListEntry, right: RunListEntry): number {
  const leftStartedAtMs = left.startedAtMs ?? null;
  const rightStartedAtMs = right.startedAtMs ?? null;

  if (
    leftStartedAtMs !== null &&
    rightStartedAtMs !== null &&
    leftStartedAtMs !== rightStartedAtMs
  ) {
    return leftStartedAtMs < rightStartedAtMs ? -1 : 1;
  }

  return left.id.localeCompare(right.id);
}
