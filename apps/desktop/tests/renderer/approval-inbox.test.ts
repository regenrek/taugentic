import { describe, expect, it, vi } from "vite-plus/test";

import type { ApprovalRequest } from "../../packages/shared/generated/index.js";

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
let approvalsState = makeQueryView<ApprovalRequest[]>([]);
const decideApproval = vi.fn(async () => ({}));

vi.mock("../../packages/renderer/src/lib/queries/session-queries.js", () => ({
  useSessionApprovalsQuery: () => approvalsState,
}));

vi.mock("../../packages/renderer/src/lib/queries/session-mutations.js", () => ({
  useDecideApprovalMutation: () => ({
    mutateAsync: decideApproval,
  }),
}));

const { ApprovalInbox } =
  await import("../../packages/renderer/src/features/approval-inbox/index.js");

describe("ApprovalInbox", () => {
  it("renders approval targets and TTL from server timestamps", async () => {
    approvalsState = makeQueryView([
      approval({
        id: "approval-tool",
        target: { kind: "toolCall", toolName: "shell" },
      }),
      approval({
        id: "approval-capsule",
        target: {
          kind: "capsuleDispatch",
          childRunId: "run-child",
          workspaceScope: "worktreeWrite",
        },
      }),
    ]);

    const rendered = await renderInbox();

    expect(rendered.document.body.textContent).toContain("Tool call: shell");
    expect(rendered.document.body.textContent).toContain("Capsule dispatch: run-child");
    expect(rendered.document.body.textContent).toContain("expires in 1m");
    expect(rendered.document.querySelectorAll("[data-approval-id]")).toHaveLength(2);
    rendered.unmount();
  });

  it("calls the approve and deny mutations with approval decisions", async () => {
    approvalsState = makeQueryView([
      approval({
        id: "approval-file",
        target: { kind: "fileWrite", paths: ["src/app.ts"] },
      }),
    ]);
    decideApproval.mockClear();
    const rendered = await renderInbox();

    await currentAct(async () => {
      buttonByName(rendered.document, "approve").click();
    });
    await currentAct(async () => {
      buttonByName(rendered.document, "deny").click();
    });

    expect(decideApproval).toHaveBeenNthCalledWith(1, {
      approvalId: "approval-file",
      decision: "approved",
    });
    expect(decideApproval).toHaveBeenNthCalledWith(2, {
      approvalId: "approval-file",
      decision: "rejected",
    });
    rendered.unmount();
  });

  it("renders empty, loading, and error states", async () => {
    approvalsState = makeQueryView([]);
    let rendered = await renderInbox();
    expect(rendered.document.body.textContent).toContain("No pending approvals.");
    rendered.unmount();

    approvalsState = makeQueryView([], { isLoading: true });
    rendered = await renderInbox();
    expect(rendered.document.body.textContent).toContain("Loading pending approvals...");
    rendered.unmount();

    approvalsState = makeQueryView([], { error: new Error("daemon offline") });
    rendered = await renderInbox();
    expect(rendered.document.body.textContent).toContain("daemon offline");
    rendered.unmount();
  });

  it("formats expired server TTLs without removing rows client-side", async () => {
    approvalsState = makeQueryView([
      approval({
        id: "approval-expired",
        expiresAtMs: 999n,
        target: { kind: "networkAccess", host: "api.example.com", protocol: "https" },
      }),
    ]);

    const rendered = await renderInbox();

    expect(rendered.document.body.textContent).toContain("expired");
    expect(rendered.document.body.textContent).toContain("Network: https://api.example.com");
    expect(rendered.document.querySelector('[data-approval-id="approval-expired"]')).not.toBeNull();
    rendered.unmount();
  });
});

async function renderInbox() {
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
    root.render(createElement(ApprovalInbox, { nowMs: 1000, sessionId: "session-1" }));
  });
  return {
    document: dom.window.document,
    unmount() {
      root.unmount();
      dom.window.close();
    },
  };
}

function approval(overrides: Partial<ApprovalRequest>): ApprovalRequest {
  return {
    id: "approval-1",
    runId: "run-1",
    scope: "processExec",
    requestedAtMs: 0n,
    expiresAtMs: 61000n,
    target: { kind: "processExec", command: "echo ok" },
    reason: "policy requires approval",
    ...overrides,
  };
}

function makeQueryView<T>(
  data: T,
  overrides: Partial<{
    error: Error | null;
    isFetching: boolean;
    isLoading: boolean;
  }> = {},
) {
  return {
    data,
    error: overrides.error ?? null,
    isFetching: overrides.isFetching ?? false,
    isLoading: overrides.isLoading ?? false,
    refetch: async () => ({}),
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
