import { describe, expect, it } from "vite-plus/test";

import type { DaemonDiagnostics } from "../../packages/shared/src/contracts.js";
import { parseDaemonDiagnostics } from "../../packages/shared/src/validation.js";
import { MissionControlPanelView } from "../../packages/renderer/src/features/mission-control/index.js";
import type { WorkflowStatusResult } from "../../packages/shared/src/contracts.js";

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

function renderMissionControl(
  snapshot: DaemonDiagnostics | undefined,
  workflowStatus?: WorkflowStatusResult,
): string {
  return renderToStaticMarkup(
    createElement(MissionControlPanelView, {
      errorMessage: null,
      isLoading: false,
      snapshot,
      workflowStatus,
    }),
  );
}

function makeDiagnostics(): DaemonDiagnostics {
  return parseDaemonDiagnostics(roundTripJson(makeDiagnosticsPayload()));
}

function makeDiagnosticsPayload() {
  return {
    claimCount: 1,
    inFlightCapsuleRunCount: 2,
    inFlightRpcCount: 3,
    providerHealth: [
      {
        displayName: "Codex",
        message: null,
        providerId: "codex",
        status: "ready",
      },
    ],
    recentErrorCount: 1,
    recentErrors: [
      {
        message: "run failed safely",
        occurredAtMs: "1700000000000",
        source: "run",
      },
    ],
    sandbox: {
      appcontainer: false,
      filesystemAllowlist: true,
      helperAvailable: true,
      networkDefaultDeny: true,
      networkDestinationAllowlist: true,
      os: "macos",
      restrictedTokenJob: false,
      sandboxKind: "macos-seatbelt",
    },
    tokenUsage: {
      cachedTokens: "2000",
      completionTokens: "1345",
      modelContextWindow: "200000",
      promptTokens: "11000",
      reasoningTokens: "345",
      totalTokens: "12345",
    },
    uptimeMs: "65000",
    worktreeCount: 4,
  };
}

function roundTripJson(value: unknown): unknown {
  return JSON.parse(JSON.stringify(value)) as unknown;
}

describe("MissionControlPanelView", () => {
  it("renders daemon diagnostics and quick links", () => {
    const markup = renderMissionControl(makeDiagnostics(), {
      loaded: {
        name: "default-github-implementation",
        path: "/Users/alice/.taugentic/workflow.yaml",
        runtimeProfileCount: 3,
        sourceKind: "github_issues",
        version: 2n,
      },
      lastReload: {
        status: "reloaded",
        name: "default-github-implementation",
        prev_name: null,
        version: 2n,
      },
    });

    expect(markup).toContain("MISSION CONTROL");
    expect(markup).toContain("1m 5s");
    expect(markup).toContain("macos · macos-seatbelt");
    expect(markup).toContain("Codex · ready");
    expect(markup).toContain("Workflow Status");
    expect(markup).toContain("Token Usage");
    expect(markup).toContain("11000");
    expect(markup).toContain("1345");
    expect(markup).toContain("default-github-implementation");
    expect(markup).toContain("github_issues · 3 capsule profiles");
    expect(markup).toContain("reload reloaded");
    expect(markup).toContain("run failed safely");
    expect(markup).toContain("Run Tree");
    expect(markup).toContain("Approval Inbox");
    expect(markup).toContain("Work Inbox");
  });

  it("renders an unavailable state without a snapshot", () => {
    const markup = renderMissionControl(undefined);

    expect(markup).toContain("diagnostics unavailable");
  });
});
