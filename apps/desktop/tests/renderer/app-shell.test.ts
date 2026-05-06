import { describe, expect, it, vi } from "vite-plus/test";

import type { DaemonControlModel } from "../../packages/renderer/src/features/daemon/model.js";

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

vi.mock("../../packages/renderer/src/features/overview/index.js", () => ({
  SessionRail: () => createElement("div", { "data-probe": "session-rail" }, "session-rail-probe"),
}));
vi.mock("../../packages/renderer/src/features/agent-visualization/index.js", () => ({
  AgentVisualizationPanel: () =>
    createElement("div", { "data-probe": "session-detail" }, "session-detail-probe"),
}));
vi.mock("../../packages/renderer/src/features/activity-log/index.js", () => ({
  GlobalActivityLog: () =>
    createElement("div", { "data-probe": "activity-log" }, "activity-log-probe"),
}));
vi.mock("../../packages/renderer/src/features/agent-runtime/index.js", () => ({
  AgentRuntimePanel: () =>
    createElement("div", { "data-probe": "agent-runtime" }, "agent-runtime-probe"),
}));
vi.mock("../../packages/renderer/src/features/attention-strip/index.js", () => ({
  AttentionStrip: () =>
    createElement("div", { "data-probe": "attention-strip" }, "attention-strip-probe"),
}));
vi.mock("../../packages/renderer/src/features/inspector-cards/index.js", () => ({
  AttentionCard: () =>
    createElement("div", { "data-probe": "attention-card" }, "attention-card-probe"),
  ProviderHealthCard: () =>
    createElement("div", { "data-probe": "provider-health-card" }, "provider-health-card-probe"),
}));
vi.mock("../../packages/renderer/src/features/mission-control/index.js", () => ({
  MissionControlPanel: () =>
    createElement("div", { "data-probe": "mission-control" }, "mission-control-probe"),
}));

import { AppShell } from "../../packages/renderer/src/app/shell.js";

function createDaemonModel(overrides: Partial<DaemonControlModel> = {}): DaemonControlModel {
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
      daemonVersion: "desktop-0.0.0",
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
    ...overrides,
  };
}

function renderAppShell(
  model: DaemonControlModel,
  {
    currentSessionId = "session-42session-42",
  }: {
    currentSessionId?: string | null;
  } = {},
): string {
  return renderToStaticMarkup(
    createElement(AppShell, {
      currentSessionId,
      daemon: model,
      onRunStarted: vi.fn(),
      onSessionChange: vi.fn(),
    }),
  );
}

describe("AppShell", () => {
  it("renders all four always-visible workspace areas", () => {
    const markup = renderAppShell(createDaemonModel());

    expect(markup).toContain("session-rail-probe");
    expect(markup).toContain("session-detail-probe");
    expect(markup).toContain("mission-control-probe");
    expect(markup).toContain("agent-runtime-probe");
    expect(markup).toContain("activity-log-probe");
    expect(markup).toContain("attention-strip-probe");
  });

  it("shows the daemon mode and version in the top status strip", () => {
    const markup = renderAppShell(createDaemonModel());

    expect(markup).toContain(">local<");
    expect(markup).toContain(">idle<");
    expect(markup).toContain("desktop-0.0.0");
    expect(markup).toContain("session-42session-42");
  });

  it("renders the theme toggle button with an accessible label", () => {
    const markup = renderAppShell(createDaemonModel());

    expect(markup).toMatch(/aria-label="Switch to (dark|light) theme"/);
  });

  it("surfaces an inline degraded banner without replacing the shell when the daemon is down", () => {
    const markup = renderAppShell(
      createDaemonModel({
        errorMessage: "daemon unavailable",
        state: null,
      }),
    );

    expect(markup).toContain("daemon degraded");
    expect(markup).toContain("daemon unavailable");
    expect(markup).toContain("session-rail-probe");
    expect(markup).toContain("session-detail-probe");
    expect(markup).toContain("agent-runtime-probe");
    expect(markup).toContain("activity-log-probe");
    expect(markup).toContain("attention-strip-probe");
  });

  it("renders the shell with no session selected", () => {
    const markup = renderAppShell(createDaemonModel(), { currentSessionId: null });

    // No session selected → top bar shows the dash placeholder in the `session` field.
    expect(markup).toMatch(/session[^<]*<\/span>\s*<span[^>]*>\s*—/);
    expect(markup).toContain("session-rail-probe");
    expect(markup).toContain("session-detail-probe");
  });
});
