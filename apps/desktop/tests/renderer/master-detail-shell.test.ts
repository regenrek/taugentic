import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import type { DaemonControlModel } from "../../packages/renderer/src/features/daemon/model.js";
import type { SessionId } from "../../packages/shared/generated/index.js";
import { createTestQueryClient, withQueryClient } from "./support/with-query-client.js";

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

interface SessionRailProps {
  onSelect: (sessionId: SessionId | null) => void;
  selectedSessionId: SessionId | null;
}

interface AgentVisualizationPanelProps {
  sessionId: SessionId | null;
}

const captured = vi.hoisted(() => ({
  rail: null as SessionRailProps | null,
  detail: null as AgentVisualizationPanelProps | null,
  agentRuntimeRenders: 0,
  activityLogRenders: 0,
  attentionStripRenders: 0,
}));

vi.mock("../../packages/renderer/src/features/overview/index.js", () => ({
  SessionRail: (props: SessionRailProps) => {
    captured.rail = props;
    return createElement("div", null, "session-rail-probe");
  },
}));
vi.mock("../../packages/renderer/src/features/agent-visualization/index.js", () => ({
  AgentVisualizationPanel: (props: AgentVisualizationPanelProps) => {
    captured.detail = props;
    return createElement("div", null, "session-detail-probe");
  },
}));
vi.mock("../../packages/renderer/src/features/activity-log/index.js", () => ({
  GlobalActivityLog: () => {
    captured.activityLogRenders += 1;
    return createElement("div", null, "activity-log-probe");
  },
}));
vi.mock("../../packages/renderer/src/features/agent-runtime/index.js", () => ({
  AgentRuntimePanel: () => {
    captured.agentRuntimeRenders += 1;
    return createElement("div", null, "agent-runtime-probe");
  },
}));
vi.mock("../../packages/renderer/src/features/attention-strip/index.js", () => ({
  AttentionStrip: () => {
    captured.attentionStripRenders += 1;
    return createElement("div", null, "attention-strip-probe");
  },
}));
vi.mock("../../packages/renderer/src/features/inspector-cards/index.js", () => ({
  AttentionCard: () => createElement("div", null, "attention-card-probe"),
  ProviderHealthCard: () => createElement("div", null, "provider-health-card-probe"),
}));

import { AppShell } from "../../packages/renderer/src/app/shell.js";

function createDaemonModel(): DaemonControlModel {
  return {
    disableBackground: async () => {},
    enableBackground: async () => {},
    errorMessage: null,
    pendingAction: null,
    reconcile: async () => {},
    refresh: async () => {},
    start: async () => {},
    state: {
      actualMode: "local",
      allowedActions: ["stop", "enableBackground"],
      backgroundOptIn: false,
      daemonVersion: null,
      desiredMode: "local",
      errorCode: null,
      message: "Local mode is the desired runtime.",
      pendingTransition: null,
      protocolVersion: "2026-04-stage2",
      reconcileRequired: false,
      logPath: "/tmp/ta-daemon.log.jsonl",
      socketPath: "/tmp/ta-daemon.sock",
      transitionStatus: "idle",
    },
    stop: async () => {},
  };
}

describe("AppShell master-detail wiring", () => {
  afterEach(() => {
    captured.rail = null;
    captured.detail = null;
    captured.agentRuntimeRenders = 0;
    captured.activityLogRenders = 0;
    captured.attentionStripRenders = 0;
  });

  it("passes null sessionId to the rail and the detail surface when no session is selected", () => {
    const onSessionChange = vi.fn();

    renderToStaticMarkup(
      withQueryClient(
        createTestQueryClient(),
        createElement(AppShell, {
          currentSessionId: null,
          daemon: createDaemonModel(),
          onRunStarted: vi.fn(),
          onSessionChange,
        }),
      ),
    );

    expect(captured.rail).not.toBeNull();
    expect(captured.rail?.selectedSessionId).toBeNull();
    expect(captured.rail?.onSelect).toBe(onSessionChange);

    expect(captured.detail).not.toBeNull();
    expect(captured.detail?.sessionId).toBeNull();

    expect(captured.agentRuntimeRenders).toBe(1);
    expect(captured.activityLogRenders).toBe(1);
    expect(captured.attentionStripRenders).toBe(1);
  });

  it("propagates the selected sessionId into both the rail and the detail surface", () => {
    const onSessionChange = vi.fn();
    const sessionId = "session-abcdef01session-abcdef01" as SessionId;

    renderToStaticMarkup(
      withQueryClient(
        createTestQueryClient(),
        createElement(AppShell, {
          currentSessionId: sessionId,
          daemon: createDaemonModel(),
          onRunStarted: vi.fn(),
          onSessionChange,
        }),
      ),
    );

    expect(captured.rail?.selectedSessionId).toBe(sessionId);
    expect(captured.rail?.onSelect).toBe(onSessionChange);
    expect(captured.detail?.sessionId).toBe(sessionId);
    expect(captured.agentRuntimeRenders).toBe(1);
  });
});
