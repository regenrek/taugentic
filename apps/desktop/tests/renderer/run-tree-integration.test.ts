/// <reference lib="dom" />
// @vitest-environment jsdom

import { fireEvent, screen, waitFor, within } from "@testing-library/dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type {
  CapsuleResult,
  ListNativeRunsResult,
  RunDetail,
  RunListEntry,
  SessionId,
} from "../../packages/shared/src/contracts.js";
import { NATIVE_RUN_LIST_MAX_LIMIT } from "../../packages/shared/src/contracts.js";
import { queryKeys } from "../../packages/renderer/src/lib/queries/keys.js";
import { createTestQueryClient, withQueryClient } from "./support/with-query-client.js";

const SESSION_ID = "session-run-tree" satisfies SessionId;
const NATIVE_RUNS_REQUEST = { limit: NATIVE_RUN_LIST_MAX_LIMIT } as const;

const apiMocks = vi.hoisted(() => ({
  detailsByRunId: new Map<string, RunDetail>(),
  getActivityPage: vi.fn(),
  getRunDetail: vi.fn(),
  getSessionOverview: vi.fn(),
  listApprovals: vi.fn(),
  listArtifacts: vi.fn(),
  listNativeRuns: vi.fn(),
  listRuns: vi.fn(),
  listSessions: vi.fn(),
}));

vi.mock("../../packages/renderer/src/lib/ipc/api.js", () => ({
  getActivityPage: apiMocks.getActivityPage,
  getRunDetail: apiMocks.getRunDetail,
  getSessionOverview: apiMocks.getSessionOverview,
  listApprovals: apiMocks.listApprovals,
  listArtifacts: apiMocks.listArtifacts,
  listNativeRuns: apiMocks.listNativeRuns,
  listRuns: apiMocks.listRuns,
  listSessions: apiMocks.listSessions,
}));

type CreateElementFn = (
  component: unknown,
  props?: Record<string, unknown> | null,
  ...children: unknown[]
) => unknown;
type ActFn = (callback: () => void | Promise<void>) => Promise<void>;
type CreateRootFn = (container: Element | DocumentFragment) => RootLike;

interface RootLike {
  render(node: unknown): void;
  unmount(): void;
}

interface QueryClientLike {
  clear(): void;
  setQueryData(queryKey: readonly unknown[], data: unknown): void;
}

const reactModulePath = "../../packages/renderer/node_modules/react/index.js";
const reactDomClientModulePath = "../../packages/renderer/node_modules/react-dom/client.js";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const { act, createElement } = (await import(reactModulePath)) as {
  act: ActFn;
  createElement: CreateElementFn;
};
const { createRoot } = (await import(reactDomClientModulePath)) as {
  createRoot: CreateRootFn;
};
const { RunTreeSection } = await import("../../packages/renderer/src/features/run-tree/index.js");

const mountedRoots: RootLike[] = [];

beforeEach(() => {
  apiMocks.detailsByRunId.clear();
  apiMocks.listNativeRuns.mockResolvedValue({
    nextCursor: null,
    runs: [],
  } satisfies ListNativeRunsResult);
  apiMocks.getRunDetail.mockImplementation(async (_sessionId: SessionId, runId: string) => {
    return apiMocks.detailsByRunId.get(runId) ?? null;
  });
});

afterEach(async () => {
  for (const root of mountedRoots.splice(0)) {
    await act(() => {
      root.unmount();
    });
  }
  document.body.innerHTML = "";
  vi.clearAllMocks();
});

describe("RunTreeSection query integration", () => {
  it("renders injected run snapshots, opens details on click, and preserves selection across polls", async () => {
    const client = createClient();
    const parent = makeRun("run-parent", {
      objectivePreview: "Parent build",
      recipeId: "parent-recipe",
      startedAtMs: 1_000n,
      status: "running",
    });
    cacheNativeRuns(client, [parent]);

    await renderRunTree(client);

    const tree = screen.getByRole("tree", { name: "Native run hierarchy" });
    expect(within(tree).getByRole("treeitem", { name: /Parent build/ })).toBeTruthy();
    expect(screen.queryByRole("treeitem", { name: /Child review/ })).toBeNull();

    const child = makeRun("run-child", {
      objectivePreview: "Child review",
      parentRunId: parent.id,
      recipeId: "debug-agent",
      startedAtMs: 2_000n,
      status: "running",
    });
    apiMocks.detailsByRunId.set(child.id, makeDetail(child, "child-payload"));

    await updateNativeRuns(client, [parent, child]);

    const childItem = await screen.findByRole("treeitem", { name: /Child review/ });
    expect(within(tree).getByText("[debug-agent]")).toBeTruthy();

    await act(() => {
      fireEvent.click(childItem);
    });

    const detailPanel = await screen.findByRole("complementary", { name: "Run detail" });
    await waitFor(() => {
      expect(within(detailPanel).getByText("CapsuleResult")).toBeTruthy();
    });
    expect(detailPanel.getAttribute("data-run-id")).toBe(child.id);
    expect(within(detailPanel).getByText("debug-agent")).toBeTruthy();
    expect(within(detailPanel).getByText(/child-payload/)).toBeTruthy();
    expect(apiMocks.getRunDetail).toHaveBeenCalledWith(SESSION_ID, child.id);

    const completedChild = {
      ...child,
      endedAtMs: 4_000n,
      status: "completed",
    } satisfies RunListEntry;
    const sibling = makeRun("run-sibling", {
      objectivePreview: "Sibling smoke",
      parentRunId: parent.id,
      startedAtMs: 3_000n,
      status: "running",
    });

    await updateNativeRuns(client, [parent, completedChild, sibling]);

    await waitFor(() => {
      expect(
        screen.getByRole("treeitem", { name: /Child review/ }).getAttribute("aria-selected"),
      ).toBe("true");
    });
    expect(
      screen.getByRole("complementary", { name: "Run detail" }).getAttribute("data-run-id"),
    ).toBe(child.id);
    expect(await screen.findByRole("treeitem", { name: /Sibling smoke/ })).toBeTruthy();
  });

  it("keeps an orphan run clickable and selected when its parent arrives late", async () => {
    const client = createClient();
    const orphan = makeRun("run-orphan", {
      objectivePreview: "Orphan task",
      parentRunId: "run-late-parent",
      recipeId: "late-lineage",
      status: "failed",
    });
    apiMocks.detailsByRunId.set(orphan.id, makeDetail(orphan, "orphan-payload"));
    cacheNativeRuns(client, [orphan]);

    await renderRunTree(client);

    const orphanTree = screen.getByRole("tree", { name: "Orphan runs" });
    const orphanItem = within(orphanTree).getByRole("treeitem", { name: /Orphan task/ });
    await act(() => {
      fireEvent.click(orphanItem);
    });

    const detailPanel = await screen.findByRole("complementary", { name: "Run detail" });
    await waitFor(() => {
      expect(within(detailPanel).getByText(/orphan-payload/)).toBeTruthy();
    });
    expect(detailPanel.getAttribute("data-run-id")).toBe(orphan.id);

    const lateParent = makeRun("run-late-parent", {
      objectivePreview: "Late parent",
      status: "completed",
    });
    await updateNativeRuns(client, [lateParent, orphan]);

    await waitFor(() => {
      expect(screen.queryByText("Orphan runs")).toBeNull();
    });
    expect(
      screen.getByRole("treeitem", { name: /Orphan task/ }).getAttribute("aria-selected"),
    ).toBe("true");
    expect(
      screen.getByRole("complementary", { name: "Run detail" }).getAttribute("data-run-id"),
    ).toBe(orphan.id);
  });

  it("supports keyboard tree navigation without losing the selected detail", async () => {
    const client = createClient();
    const parent = makeRun("run-keyboard-parent", {
      objectivePreview: "Keyboard parent",
      status: "running",
    });
    const child = makeRun("run-keyboard-child", {
      objectivePreview: "Keyboard child",
      parentRunId: parent.id,
      status: "completed",
    });
    apiMocks.detailsByRunId.set(child.id, makeDetail(child, "keyboard-payload"));
    cacheNativeRuns(client, [parent, child]);

    await renderRunTree(client);

    const parentItem = screen.getByRole("treeitem", { name: /Keyboard parent/ });
    parentItem.focus();
    await act(() => {
      fireEvent.keyDown(parentItem, { key: "ArrowDown" });
    });

    const childItem = screen.getByRole("treeitem", { name: /Keyboard child/ });
    await waitFor(() => {
      expect(document.activeElement).toBe(childItem);
      expect(childItem.getAttribute("aria-selected")).toBe("true");
    });

    await act(() => {
      fireEvent.keyDown(childItem, { key: "Enter" });
    });

    const detailPanel = await screen.findByRole("complementary", { name: "Run detail" });
    await waitFor(() => {
      expect(within(detailPanel).getByText(/keyboard-payload/)).toBeTruthy();
    });
    expect(detailPanel.getAttribute("data-run-id")).toBe(child.id);
  });
});

function createClient(): QueryClientLike {
  return createTestQueryClient() as QueryClientLike;
}

async function renderRunTree(client: QueryClientLike): Promise<void> {
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);
  mountedRoots.push(root);

  await act(() => {
    root.render(withQueryClient(client, createElement(RunTreeSection, { sessionId: SESSION_ID })));
  });
}

async function updateNativeRuns(client: QueryClientLike, runs: RunListEntry[]): Promise<void> {
  await act(() => {
    cacheNativeRuns(client, runs);
  });
}

function cacheNativeRuns(client: QueryClientLike, runs: RunListEntry[]): void {
  client.setQueryData(queryKeys.sessionNativeRuns(SESSION_ID, NATIVE_RUNS_REQUEST), {
    nextCursor: null,
    runs,
  } satisfies ListNativeRunsResult);
}

function makeRun(id: string, overrides: Partial<RunListEntry> = {}): RunListEntry {
  return {
    harness: "native",
    id,
    outputContract: "custom",
    recipeId: null,
    status: "running",
    ...overrides,
  };
}

function makeDetail(run: RunListEntry, payload: string): RunDetail {
  const result: CapsuleResult = {
    kind: "custom",
    value: {
      payload,
    },
  };

  return {
    contractViolation: null,
    outputContract: run.outputContract ?? null,
    parentRunId: run.parentRunId ?? null,
    quarantineReceipt: null,
    recipeId: run.recipeId ?? null,
    result,
    summary: {
      id: run.id,
      objective: run.objectivePreview ?? run.id,
      runtimeProfileId: "runtime-openai-safe",
      status: run.status,
    },
  };
}
