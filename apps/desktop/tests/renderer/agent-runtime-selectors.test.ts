import { describe, expect, it } from "vite-plus/test";

import type {
  AgentRuntimeStrategyInfo,
  AgentRuntimeSnapshot,
  RuntimeProfileSummary,
} from "../../packages/shared/generated/index.js";
import {
  describeProviderHealth,
  filterRuntimeProfiles,
  groupRuntimeProfilesByProvider,
} from "../../packages/renderer/src/features/agent-runtime/selectors.js";

function provider(overrides: Partial<AgentRuntimeStrategyInfo>): AgentRuntimeStrategyInfo {
  return {
    id: overrides.id ?? "openai",
    displayName: overrides.displayName ?? "OpenAI",
    models: overrides.models ?? [],
    modelCapability: overrides.modelCapability ?? {
      availability: "enumerated",
      canSetModel: true,
      currentModelId: null,
      detail: null,
    },
    health: overrides.health ?? { status: "ready", message: null },
  };
}

function profile(overrides: Partial<RuntimeProfileSummary>): RuntimeProfileSummary {
  return {
    id: overrides.id ?? "profile-1",
    displayName: overrides.displayName ?? "Default",
    providerId: overrides.providerId ?? "openai",
    modelId: overrides.modelId ?? null,
    authProfileId: overrides.authProfileId ?? null,
    policyMode: overrides.policyMode ?? "requireApproval",
  };
}

function snapshot(overrides: Partial<AgentRuntimeSnapshot> = {}): AgentRuntimeSnapshot {
  return {
    selection: overrides.selection ?? { runtimeProfileId: "profile-1" },
    providers: overrides.providers,
    runtimeProfiles: overrides.runtimeProfiles,
    authProfiles: overrides.authProfiles,
    runtimeExtensions: overrides.runtimeExtensions,
  };
}

describe("filterRuntimeProfiles", () => {
  const providers = [
    provider({ id: "openai", displayName: "OpenAI", health: { status: "ready", message: null } }),
    provider({
      id: "anthropic",
      displayName: "Anthropic",
      health: { status: "degraded", message: "rate limited" },
    }),
    provider({
      id: "codex-acp",
      displayName: "Codex (ACP)",
      health: { status: "unavailable", message: "binary missing" },
    }),
  ];

  const profiles = [
    profile({ id: "p-openai-default", displayName: "OpenAI · Default", providerId: "openai" }),
    profile({
      id: "p-anthropic-default",
      displayName: "Anthropic · Default",
      providerId: "anthropic",
    }),
    profile({ id: "p-codex-default", displayName: "Codex · Default", providerId: "codex-acp" }),
  ];

  const snap = snapshot({ providers, runtimeProfiles: profiles });

  it("returns all profiles with an empty query and readiness=any", () => {
    expect(filterRuntimeProfiles(snap, { query: "", readiness: "any" }).map((p) => p.id)).toEqual([
      "p-openai-default",
      "p-anthropic-default",
      "p-codex-default",
    ]);
  });

  it("filters by text against displayName, providerId, and provider display name", () => {
    expect(
      filterRuntimeProfiles(snap, { query: "anthropic", readiness: "any" }).map((p) => p.id),
    ).toEqual(["p-anthropic-default"]);
    expect(
      filterRuntimeProfiles(snap, { query: "ACP", readiness: "any" }).map((p) => p.id),
    ).toEqual(["p-codex-default"]);
    expect(
      filterRuntimeProfiles(snap, { query: "openai", readiness: "any" }).map((p) => p.id),
    ).toEqual(["p-openai-default"]);
  });

  it("readyOnly hides profiles whose provider is not ready", () => {
    expect(
      filterRuntimeProfiles(snap, { query: "", readiness: "readyOnly" }).map((p) => p.id),
    ).toEqual(["p-openai-default"]);
  });

  it("combines query + readyOnly", () => {
    expect(
      filterRuntimeProfiles(snap, { query: "anthropic", readiness: "readyOnly" }).map((p) => p.id),
    ).toEqual([]);
  });

  it("handles undefined snapshot gracefully", () => {
    expect(filterRuntimeProfiles(undefined, { query: "", readiness: "any" })).toEqual([]);
  });
});

describe("groupRuntimeProfilesByProvider", () => {
  const providers = [
    provider({ id: "openai", displayName: "OpenAI" }),
    provider({
      id: "anthropic",
      displayName: "Anthropic",
      health: { status: "degraded", message: null },
    }),
  ];

  it("groups profiles by providerId in snapshot-order and reports provider health", () => {
    const profiles = [
      profile({ id: "p-a", providerId: "anthropic" }),
      profile({ id: "p-o1", providerId: "openai" }),
      profile({ id: "p-o2", providerId: "openai" }),
    ];
    const groups = groupRuntimeProfilesByProvider(
      snapshot({ providers, runtimeProfiles: profiles }),
      profiles,
    );
    expect(groups.map((g) => g.providerId)).toEqual(["openai", "anthropic"]);
    expect(groups[0]!.profiles.map((p) => p.id)).toEqual(["p-o1", "p-o2"]);
    expect(groups[0]!.providerHealthStatus).toBe("ready");
    expect(groups[1]!.profiles.map((p) => p.id)).toEqual(["p-a"]);
    expect(groups[1]!.providerHealthStatus).toBe("degraded");
  });

  it("places profiles for unknown providers at the end with status=unknown", () => {
    const profiles = [
      profile({ id: "p-o1", providerId: "openai" }),
      profile({ id: "p-mystery", providerId: "unknown-xyz" }),
    ];
    const groups = groupRuntimeProfilesByProvider(snapshot({ providers }), profiles);
    expect(groups.map((g) => g.providerId)).toEqual(["openai", "unknown-xyz"]);
    expect(groups[1]!.providerHealthStatus).toBe("unknown");
    expect(groups[1]!.providerDisplayName).toBe("unknown-xyz");
  });
});

describe("describeProviderHealth", () => {
  it("returns 'missing' when no provider is selected", () => {
    expect(describeProviderHealth(null)).toEqual({
      tone: "missing",
      label: "no provider selected",
      detail: null,
    });
  });

  it("maps each health status to a tone + preserves the daemon detail message", () => {
    expect(
      describeProviderHealth(provider({ health: { status: "ready", message: null } })),
    ).toEqual({
      tone: "ready",
      label: "ready",
      detail: null,
    });
    expect(
      describeProviderHealth(provider({ health: { status: "degraded", message: "rate limited" } })),
    ).toEqual({
      tone: "degraded",
      label: "degraded",
      detail: "rate limited",
    });
    expect(
      describeProviderHealth(
        provider({ health: { status: "unavailable", message: "binary missing" } }),
      ),
    ).toEqual({
      tone: "unavailable",
      label: "unavailable",
      detail: "binary missing",
    });
    expect(
      describeProviderHealth(provider({ health: { status: "unknown", message: null } })),
    ).toEqual({
      tone: "unknown",
      label: "unknown",
      detail: null,
    });
  });

  it("normalizes blank detail messages to null", () => {
    expect(
      describeProviderHealth(provider({ health: { status: "ready", message: "   " } })).detail,
    ).toBeNull();
  });
});
