import { describe, expect, it, vi } from "vite-plus/test";

import type { SessionId, SessionOverview } from "../../packages/shared/generated/index.js";
import {
  SessionRailItem,
  SessionRailView,
} from "../../packages/renderer/src/features/overview/index.js";
import { createTestQueryClient, withQueryClient } from "./support/with-query-client.js";

type CreateElementFn = (component: unknown, props?: unknown, ...children: unknown[]) => unknown;
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

interface SessionRailViewProps {
  errorMessage: string | null;
  hasLoaded: boolean;
  hasInitialError: boolean;
  isInitialLoading: boolean;
  onSelect: (sessionId: SessionId | null) => void;
  selectedSessionId: SessionId | null;
  sessions: SessionOverview[];
}

function makeOverview(overrides: Partial<SessionOverview> = {}): SessionOverview {
  return {
    approvalAttention: "idle",
    isActive: false,
    laneStatus: "active",
    pendingApprovalCount: 0,
    session: {
      id: overrides.session?.id ?? "session-1",
      status: "running",
      title: overrides.session?.title ?? "Session 1",
    },
    ...overrides,
  };
}

function findByDataSessionId(node: unknown, sessionId: string): ReactLikeElement | null {
  if (Array.isArray(node)) {
    for (const child of node) {
      const match = findByDataSessionId(child, sessionId);
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
  if (element.props?.["data-session-id"] === sessionId) {
    return element;
  }
  if (element.props?.children !== undefined) {
    return findByDataSessionId(element.props.children, sessionId);
  }
  return null;
}

function renderSessionRailView(props: SessionRailViewProps): string {
  return renderToStaticMarkup(
    withQueryClient(createTestQueryClient(), createElement(SessionRailView, props)),
  );
}

describe("SessionRailView", () => {
  it("renders session rows sorted by operator attention priority", () => {
    const waiting = makeOverview({
      laneStatus: "waitingForApproval",
      lastActivityAtMs: 200n,
      lastEventPreview: "approval requested",
      pendingApprovalCount: 3,
      session: { id: "session-waiting", status: "paused", title: "Waiting session" },
    });
    const active = makeOverview({
      laneStatus: "active",
      lastActivityAtMs: 100n,
      lastEventPreview: "agent running",
      session: { id: "session-active", status: "running", title: "Active session" },
    });

    const markup = renderSessionRailView({
      errorMessage: null,
      hasLoaded: true,
      hasInitialError: false,
      isInitialLoading: false,
      onSelect: vi.fn(),
      selectedSessionId: "session-waiting",
      sessions: [waiting, active],
    });

    expect(markup).toContain("Waiting session");
    expect(markup).toContain("Active session");
    expect(markup).toContain("session-waiting");
    expect(markup).toContain("session-active");
    expect(markup).toContain("approval requested");
    expect(markup).toContain("agent running");
    expect(markup).toContain('data-lane-status="waitingForApproval"');
    expect(markup).toContain('data-lane-status="active"');
    expect(markup).toContain('role="listbox"');
    expect(markup).toContain('role="option"');
    expect(markup).toContain('aria-selected="true"');

    const waitingBeforeActive =
      markup.indexOf("Waiting session") < markup.indexOf("Active session");
    expect(waitingBeforeActive).toBe(true);

    const pendingBadge = markup.match(/>3</);
    expect(pendingBadge).not.toBeNull();
  });

  it("renders an empty state when sessions are empty and the model is loaded", () => {
    const markup = renderSessionRailView({
      errorMessage: null,
      hasLoaded: true,
      hasInitialError: false,
      isInitialLoading: false,
      onSelect: vi.fn(),
      selectedSessionId: null,
      sessions: [],
    });

    expect(markup).toContain("No sessions yet");
    expect(markup).toContain('data-state="empty"');
  });

  it("renders an error state when the model has an error and not yet loaded", () => {
    const markup = renderSessionRailView({
      errorMessage: "transport down",
      hasLoaded: false,
      hasInitialError: true,
      isInitialLoading: false,
      onSelect: vi.fn(),
      selectedSessionId: null,
      sessions: [],
    });

    expect(markup).toContain('data-state="error"');
    expect(markup).toContain("error: transport down");
  });

  it("renders a loading row during the first refresh", () => {
    const markup = renderSessionRailView({
      errorMessage: null,
      hasLoaded: false,
      hasInitialError: false,
      isInitialLoading: true,
      onSelect: vi.fn(),
      selectedSessionId: null,
      sessions: [],
    });

    expect(markup).toContain('data-state="loading"');
    expect(markup).toContain("Loading sessions");
  });

  it("invokes onSelect with the canonical id when a rail item handler fires", () => {
    const onSelect = vi.fn<(sessionId: SessionId | null) => void>();
    const overview = makeOverview({
      session: { id: "session-click-7", status: "running", title: "Clickable" },
    });

    const tree = (SessionRailItem as unknown as (props: unknown) => unknown)({
      overview,
      selected: false,
      tabIndex: 0,
      onSelect,
    });

    const row = findByDataSessionId(tree, "session-click-7");
    expect(row).not.toBeNull();
    const onClick = row?.props?.onClick;
    expect(typeof onClick).toBe("function");
    (onClick as () => void)();

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith("session-click-7");
  });

  it("marks the currently selected row with aria-selected=true", () => {
    const one = makeOverview({
      session: { id: "session-a", status: "running", title: "One" },
    });
    const two = makeOverview({
      session: { id: "session-b", status: "running", title: "Two" },
    });

    const markup = renderSessionRailView({
      errorMessage: null,
      hasLoaded: true,
      hasInitialError: false,
      isInitialLoading: false,
      onSelect: vi.fn(),
      selectedSessionId: "session-b",
      sessions: [one, two],
    });

    const selectedRowPattern =
      /<div\b[^>]*aria-selected="true"[^>]*data-session-id="session-b"[^>]*>/;
    const unselectedRowPattern =
      /<div\b[^>]*aria-selected="false"[^>]*data-session-id="session-a"[^>]*>/;
    expect(selectedRowPattern.test(markup)).toBe(true);
    expect(unselectedRowPattern.test(markup)).toBe(true);
    expect(markup).toContain('data-selected="true"');
  });
});
