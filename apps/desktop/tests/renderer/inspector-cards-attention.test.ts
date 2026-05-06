import { describe, expect, it, vi } from "vite-plus/test";

import type { SessionId, SessionOverviewResult } from "../../packages/shared/generated/index.js";
import {
  ATTENTION_CARD_MAX_ROWS,
  AttentionCardView,
  AttentionRow,
  deriveAttentionItems,
  totalPendingApprovals,
} from "../../packages/renderer/src/features/inspector-cards/index.js";

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

interface ReactLikeElement {
  type?: unknown;
  props?: Record<string, unknown> & { children?: unknown };
}

function findByDataAttribute(
  node: unknown,
  attribute: string,
  value?: string,
): ReactLikeElement | null {
  if (Array.isArray(node)) {
    for (const child of node) {
      const match = findByDataAttribute(child, attribute, value);
      if (match !== null) {
        return match;
      }
    }
    return null;
  }
  if (node === null || node === undefined || typeof node !== "object") {
    return null;
  }
  const element = node as ReactLikeElement;
  const attrValue = element.props?.[attribute];
  if (value === undefined ? attrValue !== undefined : attrValue === value) {
    return element;
  }
  if (element.props?.children !== undefined) {
    return findByDataAttribute(element.props.children, attribute, value);
  }
  return null;
}

function makeOverview(overrides: {
  id: string;
  title?: string;
  pendingApprovalCount: number;
  lastActivityAtMs?: bigint;
  preview?: string;
}): NonNullable<SessionOverviewResult["sessions"]>[number] {
  return {
    approvalAttention: overrides.pendingApprovalCount > 0 ? "pending" : "idle",
    isActive: overrides.pendingApprovalCount > 0,
    laneStatus: overrides.pendingApprovalCount > 0 ? "waitingForApproval" : "idle",
    lastActivityAtMs: overrides.lastActivityAtMs,
    lastEventPreview: overrides.preview ?? null,
    pendingApprovalCount: overrides.pendingApprovalCount,
    session: {
      id: overrides.id,
      status: "running",
      title: overrides.title ?? `Session ${overrides.id}`,
    },
  };
}

function makeResult(sessions: SessionOverviewResult["sessions"]): SessionOverviewResult {
  return { sessions };
}

describe("AttentionCardView", () => {
  it("renders the empty state when there are no pending approvals", () => {
    const markup = renderToStaticMarkup(
      createElement(AttentionCardView, {
        errorMessage: null,
        hasLoaded: true,
        isLoading: false,
        items: [],
        onFocusSession: vi.fn(),
        totalPending: 0,
      }),
    );

    expect(markup).toContain('data-state="empty"');
    expect(markup).toContain("no pending approvals");
    expect(markup).toContain("0 pending");
  });

  it("renders one row per pending session and surfaces total counts in the header", () => {
    const result = makeResult([
      makeOverview({
        id: "session-a",
        pendingApprovalCount: 3,
        lastActivityAtMs: 200n,
        preview: "approval requested",
        title: "Session A",
      }),
      makeOverview({
        id: "session-b",
        pendingApprovalCount: 1,
        lastActivityAtMs: 400n,
        preview: "tool call",
        title: "Session B",
      }),
      makeOverview({
        id: "session-c",
        pendingApprovalCount: 0,
        lastActivityAtMs: 100n,
        preview: "idle",
        title: "Session C",
      }),
    ]);

    const items = deriveAttentionItems(result);
    expect(items.map((item) => item.sessionId)).toEqual(["session-a", "session-b"]);

    const markup = renderToStaticMarkup(
      createElement(AttentionCardView, {
        errorMessage: null,
        hasLoaded: true,
        isLoading: false,
        items,
        onFocusSession: vi.fn(),
        totalPending: totalPendingApprovals(result),
      }),
    );

    expect(markup).toContain('data-state="ready"');
    expect(markup).toContain("4 pending");
    expect(markup).toContain('data-session-id="session-a"');
    expect(markup).toContain('data-session-id="session-b"');
    expect(markup).not.toContain('data-session-id="session-c"');

    const aIndex = markup.indexOf('data-session-id="session-a"');
    const bIndex = markup.indexOf('data-session-id="session-b"');
    expect(aIndex).toBeGreaterThanOrEqual(0);
    expect(bIndex).toBeGreaterThan(aIndex);
  });

  it("invokes onFocusSession with the session id when the focus action fires", () => {
    const onFocusSession = vi.fn<(sessionId: SessionId) => void>();
    const items = deriveAttentionItems(
      makeResult([
        makeOverview({
          id: "session-focus",
          pendingApprovalCount: 2,
          title: "Focusable",
        }),
      ]),
    );
    expect(items.length).toBe(1);

    const rowTree = (AttentionRow as unknown as (props: unknown) => unknown)({
      item: items[0]!,
      onFocus: onFocusSession,
    });

    const focusButton = findByDataAttribute(rowTree, "data-attention-focus");
    expect(focusButton).not.toBeNull();
    const onClick = focusButton?.props?.onClick as (() => void) | undefined;
    expect(typeof onClick).toBe("function");
    onClick?.();

    expect(onFocusSession).toHaveBeenCalledTimes(1);
    expect(onFocusSession).toHaveBeenCalledWith("session-focus");
  });

  it("caps the row count at ATTENTION_CARD_MAX_ROWS", () => {
    const sessions = Array.from({ length: ATTENTION_CARD_MAX_ROWS + 3 }).map((_, index) =>
      makeOverview({
        id: `session-${index.toString().padStart(2, "0")}`,
        pendingApprovalCount: index + 1,
        title: `Session ${index}`,
      }),
    );
    const items = deriveAttentionItems(makeResult(sessions));
    expect(items.length).toBe(ATTENTION_CARD_MAX_ROWS);
  });
});
