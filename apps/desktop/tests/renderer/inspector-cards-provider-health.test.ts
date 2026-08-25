import { describe, expect, it } from "vite-plus/test";

import type { AgentRuntimeSnapshot } from "../../packages/shared/generated/index.js";
import { ProviderHealthCardView } from "../../packages/renderer/src/features/inspector-cards/index.js";

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

function makeSnapshot(): AgentRuntimeSnapshot {
  return {
    selection: { runtimeProfileId: "runtime-codex-safe" },
    providers: [
      {
        id: "codex",
        displayName: "Codex",
        models: [
          {
            id: "gpt-5.6-sol",
            displayName: "GPT-5.6 Sol",
            reasoning: true,
            toolCall: true,
            structuredOutput: true,
          },
          {
            id: "gpt-5.3-codex",
            displayName: "GPT-5.3 Codex",
            reasoning: true,
            toolCall: true,
            structuredOutput: true,
          },
        ],
        modelCapability: {
          availability: "enumerated",
          canSetModel: true,
          currentModelId: "gpt-5.6-sol",
        },
        health: { status: "ready", message: null },
      },
      {
        id: "anthropic",
        displayName: "Anthropic",
        models: [
          {
            id: "claude-sonnet-4-5",
            displayName: "Claude Sonnet 4.5",
            reasoning: true,
            toolCall: true,
            structuredOutput: true,
          },
        ],
        modelCapability: {
          availability: "enumerated",
          canSetModel: true,
          currentModelId: "claude-sonnet-4-5",
        },
        health: { status: "degraded", message: "rate limited" },
      },
      {
        id: "openai",
        displayName: "OpenAI",
        models: [],
        modelCapability: { availability: "unavailable", canSetModel: false, currentModelId: null },
        health: { status: "unavailable", message: "auth failed" },
      },
    ],
  };
}

describe("ProviderHealthCardView", () => {
  it("renders one row per provider with derived tone and model counts", () => {
    const markup = renderToStaticMarkup(
      createElement(ProviderHealthCardView, {
        errorMessage: null,
        isLoading: false,
        snapshot: makeSnapshot(),
      }),
    );

    expect(markup).toContain('data-state="ready"');
    expect(markup).toContain('data-provider-id="codex"');
    expect(markup).toContain('data-provider-id="anthropic"');
    expect(markup).toContain('data-provider-id="openai"');

    expect(markup).toContain('data-provider-status="ready"');
    expect(markup).toContain('data-provider-status="degraded"');
    expect(markup).toContain('data-provider-status="unavailable"');

    expect(markup).toContain('data-tone="active"');
    expect(markup).toContain('data-tone="waiting"');
    expect(markup).toContain('data-tone="failed"');

    const codexRowMatch = markup.match(
      /data-provider-id="codex"[\s\S]*?data-provider-model-count[^>]*>([^<]+)</,
    );
    expect(codexRowMatch?.[1]).toBe("2 models");

    const openaiRowMatch = markup.match(
      /data-provider-id="openai"[\s\S]*?data-provider-model-count[^>]*>([^<]+)</,
    );
    expect(openaiRowMatch?.[1]).toBe("0 models");
  });

  it("renders the empty state when no providers are available", () => {
    const markup = renderToStaticMarkup(
      createElement(ProviderHealthCardView, {
        errorMessage: null,
        isLoading: false,
        snapshot: { selection: { runtimeProfileId: "runtime-codex-safe" } },
      }),
    );

    expect(markup).toContain('data-state="empty"');
    expect(markup).toContain("no providers");
  });

  it("renders the error state when no snapshot has loaded yet", () => {
    const markup = renderToStaticMarkup(
      createElement(ProviderHealthCardView, {
        errorMessage: "transport down",
        isLoading: false,
        snapshot: undefined,
      }),
    );

    expect(markup).toContain('data-state="error"');
    expect(markup).toContain("error: transport down");
  });

  it("renders a stale error banner once the snapshot has been observed", () => {
    const markup = renderToStaticMarkup(
      createElement(ProviderHealthCardView, {
        errorMessage: "timeout",
        isLoading: false,
        snapshot: makeSnapshot(),
      }),
    );

    expect(markup).toContain('data-state="ready"');
    expect(markup).toContain('data-state="stale"');
    expect(markup).toContain("stale · timeout");
  });
});
