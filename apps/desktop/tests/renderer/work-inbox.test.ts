import { describe, expect, it, vi } from "vite-plus/test";

import type { WorkItem, WorkItemListResult } from "../../packages/shared/generated/index.js";

type CreateElementFn = (
  component: unknown,
  props?: Record<string, unknown> | null,
  ...children: unknown[]
) => unknown;
type CreateRootFn = (container: Element) => {
  render: (element: unknown) => void;
  unmount: () => void;
};
type ActFn = (callback: () => void | Promise<void>) => Promise<void>;

const reactModulePath = "../../packages/renderer/node_modules/react/index.js";
const reactDomClientModulePath = "../../packages/renderer/node_modules/react-dom/client.js";

// @ts-expect-error jsdom ships without declarations in this workspace.
const { JSDOM } = (await import("jsdom")) as {
  JSDOM: new (
    html?: string,
    options?: { pretendToBeVisual?: boolean },
  ) => {
    window: Window & typeof globalThis;
  };
};

let currentAct: ActFn = async (callback) => {
  await callback();
};
let workItemsState = makeQuery({
  items: [],
  sync: { state: "idle" },
});
const refreshWorkItems = vi.fn();
const dismissWorkItem = vi.fn();
const triggerWorkItem = vi.fn();

vi.mock("../../packages/renderer/src/lib/queries/work-items.js", () => ({
  useWorkItemsQuery: () => workItemsState,
  useRefreshWorkItemsMutation: () => mutation(refreshWorkItems),
  useDismissWorkItemMutation: () => mutation(dismissWorkItem),
  useTriggerWorkItemMutation: () => mutation(triggerWorkItem),
}));

const { WorkInbox } = await import("../../packages/renderer/src/features/work-inbox/index.js");

describe("WorkInbox", () => {
  it("renders work items and dispatches actions through mutations", async () => {
    workItemsState = makeQuery({
      items: [workItem("github:regenrek/taugentic#1")],
      sync: { state: "idle", detail: "daemon synced" },
    });
    refreshWorkItems.mockClear();
    dismissWorkItem.mockClear();
    triggerWorkItem.mockClear();

    const rendered = await renderInbox("session-1");

    expect(rendered.document.body.textContent).toContain("work inbox");
    expect(rendered.document.body.textContent).toContain("Issue title");
    expect(rendered.document.body.textContent).toContain("daemon synced");
    await currentAct(async () => buttonByName(rendered.document, "refresh").click());
    await currentAct(async () => buttonByName(rendered.document, "trigger").click());
    await currentAct(async () => buttonByName(rendered.document, "dismiss").click());
    expect(refreshWorkItems).toHaveBeenCalledWith({});
    expect(triggerWorkItem).toHaveBeenCalledWith({ key: "github:regenrek/taugentic#1" });
    expect(dismissWorkItem).toHaveBeenCalledWith({ key: "github:regenrek/taugentic#1" });
    rendered.unmount();
  });

  it("renders empty, loading, and error states", async () => {
    workItemsState = makeQuery({ items: [], sync: { state: "idle" } });
    let rendered = await renderInbox("session-1");
    expect(rendered.document.body.textContent).toContain("No background work items.");
    rendered.unmount();

    workItemsState = makeQuery({ items: [], sync: { state: "idle" } }, { isLoading: true });
    rendered = await renderInbox("session-1");
    expect(rendered.document.body.textContent).toContain("Loading work items...");
    rendered.unmount();

    workItemsState = makeQuery(
      { items: [], sync: { state: "idle" } },
      { error: new Error("offline") },
    );
    rendered = await renderInbox("session-1");
    expect(rendered.document.body.textContent).toContain("offline");
    rendered.unmount();
  });
});

async function renderInbox(selectedSessionId: string | null) {
  const dom = new JSDOM("<!doctype html><html><body><main></main></body></html>", {
    pretendToBeVisual: true,
  });
  installDomGlobals(dom);
  const { createElement } = (await import(reactModulePath)) as { createElement: CreateElementFn };
  const { createRoot } = (await import(reactDomClientModulePath)) as { createRoot: CreateRootFn };
  const { act } = (await import(reactModulePath)) as { act: ActFn };
  currentAct = act;
  const container = dom.window.document.querySelector("main");
  if (container === null) {
    throw new Error("test container missing");
  }
  const root = createRoot(container);
  await currentAct(async () => {
    root.render(createElement(WorkInbox, { selectedSessionId }));
  });
  return {
    document: dom.window.document,
    unmount() {
      root.unmount();
      dom.window.close();
    },
  };
}

function workItem(key: string): WorkItem {
  return {
    body: "Issue body",
    externalId: "#1",
    fetchedAtMs: 100n,
    key,
    labels: ["ready"],
    source: { kind: "gitHub", repo_name: "taugentic", repo_owner: "regenrek" },
    status: "available",
    title: "Issue title",
    url: "https://github.com/regenrek/taugentic/issues/1",
  };
}

function makeQuery(
  data: WorkItemListResult,
  overrides: Partial<{ error: Error | null; isFetching: boolean; isLoading: boolean }> = {},
) {
  return {
    data,
    error: overrides.error ?? null,
    isFetching: overrides.isFetching ?? false,
    isLoading: overrides.isLoading ?? false,
  };
}

function mutation(fn: typeof refreshWorkItems) {
  return {
    error: null,
    isPending: false,
    mutate: fn,
  };
}

function installDomGlobals(dom: { window: Window & typeof globalThis }) {
  Object.assign(globalThis, {
    IS_REACT_ACT_ENVIRONMENT: true,
    document: dom.window.document,
    HTMLElement: dom.window.HTMLElement,
    MouseEvent: dom.window.MouseEvent,
    Node: dom.window.Node,
    window: dom.window,
  });
}

function buttonByName(document: Document, name: string): HTMLButtonElement {
  const match = Array.from(document.querySelectorAll<HTMLButtonElement>("button")).find(
    (button) => button.textContent?.includes(name) ?? false,
  );
  if (!match) {
    throw new Error(`button not found: ${name}`);
  }
  return match;
}
