import { describe, expect, it, vi } from "vite-plus/test";

import type { AgentRuntimeSnapshot } from "../../packages/shared/generated/index.js";
import { AgentRuntimePanelView } from "../../packages/renderer/src/features/agent-runtime/index.js";

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
    selection: {
      runtimeProfileId: "runtime-deepseek-safe",
    },
    providers: [
      {
        id: "deepseek",
        displayName: "DeepSeek",
        models: [
          {
            id: "deepseek-chat",
            displayName: "DeepSeek Chat",
            reasoning: false,
            toolCall: true,
            structuredOutput: true,
          },
        ],
        modelCapability: {
          availability: "enumerated",
          canSetModel: true,
          currentModelId: "deepseek-chat",
        },
        health: {
          status: "ready",
          message: "ready",
        },
      },
      {
        id: "codex",
        displayName: "Codex",
        models: [
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
          currentModelId: "gpt-5.3-codex",
        },
        health: {
          status: "ready",
          message: null,
        },
      },
    ],
    authProfiles: [
      {
        profile: {
          id: "auth-codex-chatgpt",
          providerId: "codex",
          displayName: "Codex ChatGPT",
        },
        connectionState: "loggedOut",
        lastError: null,
        managementMode: "interactive",
        canLogin: true,
        canLogout: true,
        setupSteps: [],
        action: null,
        methods: [
          {
            id: "chatgpt",
            displayName: "ChatGPT",
            managementMode: "interactive",
          },
        ],
      },
    ],
    runtimeProfiles: [
      {
        id: "runtime-deepseek-safe",
        displayName: "DeepSeek Safe",
        providerId: "deepseek",
        modelId: "deepseek-chat",
        authProfileId: "deepseek-api-key",
        policyMode: "requireApproval",
      },
      {
        id: "runtime-codex-safe",
        displayName: "Codex Safe",
        providerId: "codex",
        modelId: "gpt-5.6-sol",
        authProfileId: "auth-codex-chatgpt",
        policyMode: "allow",
      },
    ],
    runtimeExtensions: [
      {
        descriptor: {
          id: "local-shell-tools",
          displayName: "Local shell tools",
          description: "Run local shell tools",
        },
        availability: "available",
        enabled: true,
      },
    ],
  };
}

function renderPanel(overrides: Partial<Parameters<typeof AgentRuntimePanelView>[0]> = {}): string {
  return renderToStaticMarkup(
    createElement(AgentRuntimePanelView, {
      errorMessage: null,
      isAuthActionPending: false,
      isFetching: false,
      isLoading: false,
      isMutating: false,
      mutationErrorMessage: null,
      onAuthLogin: vi.fn(),
      onAuthLogout: vi.fn(),
      onModelChange: vi.fn(),
      onPolicyModeChange: vi.fn(),
      onRefresh: vi.fn(),
      onSelectProfile: vi.fn(),
      onSetExtensionEnabled: vi.fn(),
      snapshot: makeSnapshot(),
      ...overrides,
    }),
  );
}

describe("AgentRuntimePanelView", () => {
  it("renders the provider-filtered auth empty state", () => {
    const markup = renderPanel();

    expect(markup).toContain("No auth profiles.");
    expect(markup).not.toContain("Codex ChatGPT");
  });

  it("renders mutation failures separately from stale snapshot errors", () => {
    const markup = renderPanel({
      errorMessage: "timeout",
      mutationErrorMessage: "login failed",
    });

    expect(markup).toContain("stale · timeout");
    expect(markup).toContain("mutation failed · login failed");
  });

  it("disables auth action buttons while an auth mutation is pending", () => {
    const markup = renderPanel({
      isAuthActionPending: true,
      snapshot: {
        ...makeSnapshot(),
        selection: {
          runtimeProfileId: "runtime-codex-safe",
        },
      },
    });

    expect(markup).toContain("Codex ChatGPT");
    expect(markup).toMatch(/<button[^>]*disabled[^>]*>Login<\/button>/);
  });

  it("renders ChatGPT subscription-only status without retry guidance", () => {
    const markup = renderPanel({
      snapshot: {
        ...makeSnapshot(),
        selection: {
          runtimeProfileId: "runtime-openai-chatgpt",
        },
        providers: [
          {
            id: "openai",
            displayName: "OpenAI",
            models: [],
            modelCapability: {
              availability: "currentOnly",
              canSetModel: false,
              currentModelId: "gpt-5.6-sol",
            },
            health: {
              status: "ready",
              message: null,
            },
          },
        ],
        authProfiles: [
          {
            profile: {
              id: "auth-openai-chatgpt",
              providerId: "openai",
              displayName: "OpenAI ChatGPT Subscription",
            },
            connectionState: "connected",
            lastError: null,
            managementMode: "interactive",
            canLogin: false,
            canLogout: true,
            platformOrgLinked: false,
            setupSteps: [],
            action: null,
            methods: [],
          },
        ],
        runtimeProfiles: [
          {
            id: "runtime-openai-chatgpt",
            displayName: "OpenAI ChatGPT",
            providerId: "openai",
            modelId: "gpt-5.6-sol",
            authProfileId: "auth-openai-chatgpt",
            policyMode: "allow",
          },
        ],
      },
    });

    expect(markup).toContain("Connected · ChatGPT subscription only · Platform org not linked");
    expect(markup).toContain("https://platform.openai.com/settings/organization");
    expect(markup).not.toContain("retry login");
  });

  it("renders delegated ACP auth guidance without a fake login button", () => {
    const markup = renderPanel({
      snapshot: {
        ...makeSnapshot(),
        selection: {
          runtimeProfileId: "runtime-codex-safe",
        },
        providers: [
          {
            id: "codex",
            displayName: "Codex ACP",
            models: [],
            modelCapability: {
              availability: "currentOnly",
              canSetModel: false,
              currentModelId: "current",
            },
            health: {
              status: "degraded",
              message: "sign in required",
            },
          },
        ],
        authProfiles: [
          {
            profile: {
              id: "auth-codex-chatgpt",
              providerId: "codex",
              displayName: "Codex ACP Auth",
            },
            connectionState: "loggedOut",
            lastError: "sign in required",
            managementMode: "terminalCliDelegated",
            canLogin: false,
            canLogout: false,
            setupSteps: ["Install CLI", "Run login"],
            action: {
              label: "Authenticate",
              command: "codex login",
              description: "Run codex login outside Taugentic",
            },
            methods: [
              {
                id: "oauth",
                displayName: "OAuth",
                managementMode: "terminalCliDelegated",
              },
            ],
          },
        ],
      },
    });

    expect(markup).toContain("terminalCliDelegated");
    expect(markup).toContain("codex login");
    expect(markup).not.toContain(">Login</button>");
  });

  it("renders current-only model messaging for ACP providers", () => {
    const baseSnapshot = makeSnapshot();
    const markup = renderPanel({
      snapshot: {
        ...baseSnapshot,
        providers: [
          {
            id: "deepseek",
            displayName: "Cursor ACP",
            models: [],
            modelCapability: {
              availability: "currentOnly",
              canSetModel: false,
              currentModelId: "cursor/gpt-5",
            },
            health: {
              status: "ready",
              message: null,
            },
          },
          ...(baseSnapshot.providers ?? []).slice(1),
        ],
      },
    });

    expect(markup).toContain("current-only surface; active model cursor/gpt-5");
  });

  it("separates selected profile model from provider current model", () => {
    const baseSnapshot = makeSnapshot();
    const markup = renderPanel({
      snapshot: {
        ...baseSnapshot,
        providers: [
          {
            id: "deepseek",
            displayName: "OpenCode",
            models: [
              {
                id: "zai/glm-5.1",
                displayName: "Z.AI/GLM-5.1",
                reasoning: true,
                toolCall: true,
                structuredOutput: true,
              },
            ],
            modelCapability: {
              availability: "enumerated",
              canSetModel: true,
              currentModelId: "opencode/big-pickle",
            },
            health: {
              status: "ready",
              message: null,
            },
          },
          ...(baseSnapshot.providers ?? []).slice(1),
        ],
        runtimeProfiles: [
          {
            id: "runtime-deepseek-safe",
            displayName: "OpenCode ACP Allow",
            providerId: "deepseek",
            modelId: "zai/glm-5.1",
            authProfileId: "auth-opencode",
            policyMode: "allow",
          },
          ...(baseSnapshot.runtimeProfiles ?? []).slice(1),
        ],
      },
    });

    expect(markup).toContain("selected Z.AI/GLM-5.1");
    expect(markup).not.toContain("opencode/big-pickle");
  });

  it("handles current-only providers when the models field is omitted", () => {
    const baseSnapshot = makeSnapshot();
    const markup = renderPanel({
      snapshot: {
        ...baseSnapshot,
        providers: [
          {
            id: "deepseek",
            displayName: "Cursor ACP",
            modelCapability: {
              availability: "currentOnly",
              canSetModel: false,
              currentModelId: "cursor/gpt-5",
            },
            health: {
              status: "ready",
              message: null,
            },
          },
          ...(baseSnapshot.providers ?? []).slice(1),
        ],
      },
    });

    expect(markup).toContain("current-only surface; active model cursor/gpt-5");
  });

  it("keeps the refresh button label stable while polling fetches are in flight", () => {
    const markup = renderPanel({
      isFetching: true,
    });

    expect(markup).toContain("Refresh");
    expect(markup).not.toContain("Refreshing...");
  });
});
