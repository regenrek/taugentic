import { describe, expect, it, vi } from "vite-plus/test";

import type { PublicDaemonEventEnvelope } from "../../packages/shared/generated/index.js";
import {
  GlobalActivityLogView,
  createInitialGlobalActivityLogViewState,
} from "../../packages/renderer/src/features/activity-log/GlobalActivityLog.js";
import type { ActivityKindFilter } from "../../packages/renderer/src/features/activity-log/GlobalActivityLog.js";

type CreateElementFn = (component: unknown, props?: Record<string, unknown> | null) => unknown;
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

function makeEnvelope(
  sessionId: string,
  sequence: bigint,
  occurredAtMs: bigint,
  event: PublicDaemonEventEnvelope["event"],
): PublicDaemonEventEnvelope {
  return {
    daemonInstanceId: "daemon-1",
    event,
    occurredAtMs,
    sequence,
    sessionId,
  };
}

describe("GlobalActivityLogView", () => {
  it("renders the empty state when there are no events and no error", () => {
    const markup = renderToStaticMarkup(
      createElement(GlobalActivityLogView, {
        nowMs: 1_000_000,
        onKindFilterChange: vi.fn(),
        state: {
          ...createInitialGlobalActivityLogViewState(),
          hasLoaded: true,
        },
      }),
    );

    expect(markup).toContain("No daemon events yet.");
    expect(markup).toContain('data-state="empty"');
  });

  it("renders the error block when errorMessage is set before first load", () => {
    const markup = renderToStaticMarkup(
      createElement(GlobalActivityLogView, {
        nowMs: 1_000_000,
        onKindFilterChange: vi.fn(),
        state: {
          ...createInitialGlobalActivityLogViewState(),
          errorMessage: "transport down",
        },
      }),
    );

    expect(markup).toContain("Daemon event stream unavailable: transport down");
    expect(markup).toContain('data-state="error"');
  });

  it("renders a stale error banner after first load while keeping events visible", () => {
    const envelope = makeEnvelope("session-a", 1n, 500n, {
      run: { runId: "run-1", status: "running", detail: "tick" },
    });
    const markup = renderToStaticMarkup(
      createElement(GlobalActivityLogView, {
        nowMs: 1_000_000,
        onKindFilterChange: vi.fn(),
        state: {
          ...createInitialGlobalActivityLogViewState(),
          errorMessage: "timeout",
          events: [envelope],
          hasLoaded: true,
        },
      }),
    );

    expect(markup).toContain('data-state="stale"');
    expect(markup).toContain("stale · last refresh failed: timeout");
    expect(markup).toContain('data-kind="run"');
  });

  it("renders one row per event with the correct data-kind attribute", () => {
    const runEnvelope = makeEnvelope("session-run", 1n, 500n, {
      run: { runId: "run-1", status: "running", detail: "tick" },
    });
    const approvalEnvelope = makeEnvelope("session-approval", 2n, 900n, {
      approval: {
        phase: "requested",
        request: {
          expiresAtMs: 60_000n,
          id: "approval-1",
          requestedAtMs: 0n,
          runId: "run-1",
          scope: "networkAccess",
          target: { kind: "networkAccess", host: "api.example.com", protocol: "https" },
          reason: "needs access",
        },
      },
    });

    const markup = renderToStaticMarkup(
      createElement(GlobalActivityLogView, {
        nowMs: 1_000_000,
        onKindFilterChange: vi.fn(),
        state: {
          ...createInitialGlobalActivityLogViewState(),
          events: [approvalEnvelope, runEnvelope],
          hasLoaded: true,
        },
      }),
    );

    expect(markup).toContain('data-kind="run"');
    expect(markup).toContain('data-kind="approval"');
    expect(markup).toContain('data-session-id="session-run"');
    expect(markup).toContain('data-session-id="session-approval"');
    expect(markup).toContain("var(--accent)");
    expect(markup).toContain("var(--status-waiting)");
  });

  it("invokes onKindFilterChange with the selected value when an option is clicked", () => {
    const onKindFilterChange = vi.fn<(filter: ActivityKindFilter) => void>();

    const tree = (GlobalActivityLogView as unknown as (props: unknown) => unknown)({
      nowMs: 1_000_000,
      onKindFilterChange,
      state: {
        ...createInitialGlobalActivityLogViewState(),
        hasLoaded: true,
      },
    });

    const trigger = findByDataAttribute(tree, "data-activity-kind-filter-trigger");
    expect(trigger).not.toBeNull();

    const runOption = findByDataAttribute(tree, "data-activity-kind-filter-option", "run");
    expect(runOption).not.toBeNull();

    const onClick = runOption?.props?.onClick as (() => void) | undefined;
    expect(typeof onClick).toBe("function");
    onClick?.();

    expect(onKindFilterChange).toHaveBeenCalledTimes(1);
    expect(onKindFilterChange).toHaveBeenCalledWith("run");
  });
});
