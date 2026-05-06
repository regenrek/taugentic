import { RefreshCcw } from "lucide-react";
import { useMemo, useState } from "react";

import type {
  AgentRuntimeModelId,
  AgentRuntimeSnapshot,
  AgentRuntimeStrategyId,
  AgentRuntimeStrategyInfo,
  AuthProfileLoginChallenge,
  AuthProfileRef,
  AuthProfileState,
  RuntimePolicyMode,
  RuntimeExtensionId,
  RuntimeProfileId,
  RuntimeProfileSummary,
} from "@taugentic/desktop-shared";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  useAgentRuntimeQuery,
  useLoginAgentRuntimeAuthProfileMutation,
  useLogoutAgentRuntimeAuthProfileMutation,
  usePatchAgentRuntimeProfileMutation,
  useSelectAgentRuntimeProfileMutation,
  useSetAgentRuntimeExtensionEnabledMutation,
} from "@/lib/queries/agent-runtime";

import {
  describeProviderHealth,
  filterRuntimeProfiles,
  groupRuntimeProfilesByProvider,
  type RuntimeReadinessFilter,
} from "./selectors";

const POLICY_OPTIONS: readonly RuntimePolicyMode[] = ["requireApproval", "allow", "deny"] as const;

export function AgentRuntimePanel() {
  const runtime = useAgentRuntimeQuery();
  const selectProfile = useSelectAgentRuntimeProfileMutation();
  const patchProfile = usePatchAgentRuntimeProfileMutation();
  const loginProfile = useLoginAgentRuntimeAuthProfileMutation();
  const logoutProfile = useLogoutAgentRuntimeAuthProfileMutation();
  const setExtensionEnabled = useSetAgentRuntimeExtensionEnabledMutation();
  const [loginChallenge, setLoginChallenge] = useState<AuthProfileLoginChallenge | null>(null);

  const snapshot = runtime.data;
  const errorMessage = runtime.error ? toErrorMessage(runtime.error) : null;
  const mutationErrorMessage = firstErrorMessage(
    selectProfile.error,
    patchProfile.error,
    loginProfile.error,
    logoutProfile.error,
    setExtensionEnabled.error,
  );

  return (
    <AgentRuntimePanelView
      errorMessage={errorMessage}
      isAuthActionPending={loginProfile.isPending || logoutProfile.isPending}
      isFetching={runtime.isFetching}
      isLoading={runtime.isLoading}
      isMutating={
        selectProfile.isPending ||
        patchProfile.isPending ||
        loginProfile.isPending ||
        logoutProfile.isPending ||
        setExtensionEnabled.isPending
      }
      mutationErrorMessage={mutationErrorMessage}
      loginChallenge={loginChallenge}
      onAuthLogin={(authProfileId) =>
        loginProfile.mutate(
          { authProfileId },
          {
            onSuccess: (result) => setLoginChallenge(result.challenge ?? null),
          },
        )
      }
      onAuthLogout={(authProfileId) =>
        logoutProfile.mutate(
          { authProfileId },
          {
            onSuccess: () => setLoginChallenge(null),
          },
        )
      }
      onModelChange={(runtimeProfileId, modelId) =>
        patchProfile.mutate({
          runtimeProfileId,
          patch:
            modelId === null
              ? { modelId: { kind: "clear" } }
              : { modelId: { kind: "set", value: modelId } },
        })
      }
      onPolicyModeChange={(runtimeProfileId, policyMode) =>
        patchProfile.mutate({
          runtimeProfileId,
          patch: { policyMode },
        })
      }
      onRefresh={() => runtime.refetch()}
      onSelectProfile={(runtimeProfileId) =>
        selectProfile.mutate({
          runtimeProfileId,
        })
      }
      onSetExtensionEnabled={(extensionId, enabled) =>
        setExtensionEnabled.mutate({
          extensionId,
          enabled,
        })
      }
      snapshot={snapshot}
    />
  );
}

interface AgentRuntimePanelViewProps {
  errorMessage: string | null;
  isAuthActionPending: boolean;
  isFetching: boolean;
  isLoading: boolean;
  isMutating: boolean;
  loginChallenge: AuthProfileLoginChallenge | null;
  mutationErrorMessage: string | null;
  onAuthLogin: (authProfileId: AuthProfileRef["id"]) => void;
  onAuthLogout: (authProfileId: AuthProfileRef["id"]) => void;
  onModelChange: (runtimeProfileId: RuntimeProfileId, modelId: AgentRuntimeModelId | null) => void;
  onPolicyModeChange: (runtimeProfileId: RuntimeProfileId, policyMode: RuntimePolicyMode) => void;
  onRefresh: () => void;
  onSelectProfile: (runtimeProfileId: RuntimeProfileId) => void;
  onSetExtensionEnabled: (extensionId: RuntimeExtensionId, enabled: boolean) => void;
  snapshot: AgentRuntimeSnapshot | undefined;
}

export function AgentRuntimePanelView({
  errorMessage,
  isAuthActionPending,
  isFetching: _isFetching,
  isLoading,
  isMutating,
  loginChallenge,
  mutationErrorMessage,
  onAuthLogin,
  onAuthLogout,
  onModelChange,
  onPolicyModeChange,
  onRefresh,
  onSelectProfile,
  onSetExtensionEnabled,
  snapshot,
}: AgentRuntimePanelViewProps) {
  const selectedProfile = getSelectedProfile(snapshot);
  const selectedProvider = selectedProfile
    ? providerById(snapshot?.providers ?? [], selectedProfile.providerId)
    : null;
  const selectedModelId = selectedProfile?.modelId ?? null;
  const authProfiles = authProfilesForProvider(
    snapshot?.authProfiles ?? [],
    selectedProfile?.providerId ?? null,
  );
  const modelCapability = selectedProvider?.modelCapability ?? null;
  const models = selectedProvider?.models ?? [];
  const canChangeModel =
    selectedProfile !== null &&
    modelCapability?.availability === "enumerated" &&
    modelCapability.canSetModel;

  const [searchQuery, setSearchQuery] = useState<string>("");
  const [readinessFilter, setReadinessFilter] = useState<RuntimeReadinessFilter>("any");
  const filteredProfiles = useMemo(
    () => filterRuntimeProfiles(snapshot, { query: searchQuery, readiness: readinessFilter }),
    [snapshot, searchQuery, readinessFilter],
  );
  const profileGroups = useMemo(
    () => groupRuntimeProfilesByProvider(snapshot, filteredProfiles),
    [snapshot, filteredProfiles],
  );
  const totalProfileCount = snapshot?.runtimeProfiles?.length ?? 0;
  const selectedHealth = describeProviderHealth(selectedProvider);
  const manualBrowserUrl = loginChallenge?.manualBrowserUrl ?? null;
  const authorizeUrl = loginChallenge?.authorizeUrl ?? null;
  const secondaryAuthorizeUrl =
    authorizeUrl !== null && authorizeUrl !== manualBrowserUrl ? authorizeUrl : null;
  const selectedIsFilteredOut =
    selectedProfile !== null &&
    filteredProfiles.find((candidate) => candidate.id === selectedProfile.id) === undefined;

  return (
    <section
      aria-label="Agent runtime"
      className="border-b border-[var(--border)] bg-[var(--bg)] font-[var(--font-mono)]"
      data-feature="agent-runtime-panel"
    >
      <header className="flex items-center justify-between border-b border-[var(--border)] px-3 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-[10px] uppercase tracking-[0.18em] text-[var(--fg-mute)]">
            RUNTIME
          </span>
          {selectedProfile ? (
            <Badge className="font-mono text-[10px]" variant="outline">
              {selectedProfile.displayName}
            </Badge>
          ) : null}
        </div>
        <Button onClick={onRefresh} size="sm" type="button" variant="secondary">
          <RefreshCcw className="size-3.5" />
          Refresh
        </Button>
      </header>

      {isLoading ? (
        <div className="px-3 py-3 text-[12px] text-[var(--fg-dim)]">Loading runtime snapshot…</div>
      ) : snapshot === undefined ? (
        <div className="px-3 py-3 text-[12px] text-[var(--status-failed)]">
          error: {errorMessage ?? "runtime snapshot unavailable"}
        </div>
      ) : (
        <div className="flex flex-col gap-3 px-3 py-3">
          <ProviderHealthBanner
            detail={selectedHealth.detail}
            label={selectedHealth.label}
            tone={selectedHealth.tone}
          />

          <div className="flex flex-wrap items-center gap-2">
            <Input
              aria-label="filter runtime profiles"
              className="h-7 max-w-[220px] font-[var(--font-mono)] text-[11px]"
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="filter profiles…"
              type="search"
              value={searchQuery}
            />
            <label className="flex cursor-pointer items-center gap-1 text-[11px] text-[var(--fg-dim)]">
              <input
                checked={readinessFilter === "readyOnly"}
                className="size-3.5"
                onChange={(event) =>
                  setReadinessFilter(event.currentTarget.checked ? "readyOnly" : "any")
                }
                type="checkbox"
              />
              <span className="uppercase tracking-[0.16em]">ready only</span>
            </label>
            <span className="text-[10px] text-[var(--fg-mute)]">
              {filteredProfiles.length}/{totalProfileCount} profiles
            </span>
            {selectedIsFilteredOut ? (
              <span
                className="text-[10px] text-[var(--fg-mute)]"
                data-runtime-selection-note="hidden-by-filter"
              >
                selected profile hidden by filter
              </span>
            ) : null}
          </div>

          <div className="grid gap-2 sm:grid-cols-2">
            <label className="flex flex-col gap-1 text-[11px] text-[var(--fg-dim)]">
              <span className="uppercase tracking-[0.18em]">Runtime profile</span>
              <select
                className="border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-1.5 text-[12px] text-[var(--fg)]"
                disabled={isMutating || filteredProfiles.length === 0}
                onChange={(event) => onSelectProfile(event.currentTarget.value as RuntimeProfileId)}
                value={snapshot.selection.runtimeProfileId}
              >
                {profileGroups.length === 0 ? (
                  <option disabled value={snapshot.selection.runtimeProfileId}>
                    no matching profiles
                  </option>
                ) : (
                  profileGroups.map((group) => (
                    <optgroup
                      key={group.providerId}
                      label={`${group.providerDisplayName} · ${group.providerHealthStatus}`}
                    >
                      {group.profiles.map((profile) => (
                        <option key={profile.id} value={profile.id}>
                          {profile.displayName}
                        </option>
                      ))}
                    </optgroup>
                  ))
                )}
              </select>
            </label>

            <label className="flex flex-col gap-1 text-[11px] text-[var(--fg-dim)]">
              <span className="uppercase tracking-[0.18em]">Policy mode</span>
              <select
                className="border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-1.5 text-[12px] text-[var(--fg)]"
                disabled={isMutating || selectedProfile === null}
                onChange={(event) => {
                  if (selectedProfile === null) {
                    return;
                  }
                  onPolicyModeChange(
                    selectedProfile.id,
                    event.currentTarget.value as RuntimePolicyMode,
                  );
                }}
                value={selectedProfile?.policyMode ?? ""}
              >
                {POLICY_OPTIONS.map((policyMode) => (
                  <option key={policyMode} value={policyMode}>
                    {policyMode}
                  </option>
                ))}
              </select>
            </label>

            <label className="flex flex-col gap-1 text-[11px] text-[var(--fg-dim)]">
              <span className="uppercase tracking-[0.18em]">Provider</span>
              <div className="border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-1.5 text-[12px] text-[var(--fg)]">
                {selectedProvider?.displayName ?? "—"}
              </div>
            </label>

            <label className="flex flex-col gap-1 text-[11px] text-[var(--fg-dim)]">
              <span className="uppercase tracking-[0.18em]">Model</span>
              <select
                className="border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-1.5 text-[12px] text-[var(--fg)]"
                disabled={isMutating || !canChangeModel}
                onChange={(event) => {
                  if (selectedProfile === null) {
                    return;
                  }
                  const nextModelId = event.currentTarget.value;
                  onModelChange(
                    selectedProfile.id,
                    nextModelId === "" ? null : (nextModelId as AgentRuntimeModelId),
                  );
                }}
                value={selectedModelId ?? ""}
              >
                <option value="">
                  {modelCapability?.availability === "enumerated"
                    ? "provider default"
                    : "current / provider managed"}
                </option>
                {models.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.displayName}
                  </option>
                ))}
              </select>
              {selectedProvider ? (
                <div
                  className="text-[11px] text-[var(--fg-dim)]"
                  data-runtime-model-capability={modelCapability?.availability ?? "missing"}
                >
                  {describeModelCapability(selectedProvider, selectedModelId)}
                </div>
              ) : null}
            </label>
          </div>

          <div className="flex flex-wrap gap-2">
            {(snapshot.providers ?? []).map((provider) => (
              <Badge key={provider.id} className="font-mono text-[10px]" variant="outline">
                {provider.displayName} · {provider.health.status}
              </Badge>
            ))}
          </div>

          <div className="space-y-2">
            <div className="text-[10px] uppercase tracking-[0.18em] text-[var(--fg-mute)]">
              Auth profiles
            </div>
            {authProfiles.length === 0 ? (
              <div className="text-[12px] text-[var(--fg-dim)]">No auth profiles.</div>
            ) : (
              <div className="grid gap-2">
                {authProfiles.map((authProfile) => {
                  const authMethods = authProfile.methods ?? [];
                  const setupSteps = authProfile.setupSteps ?? [];
                  const canLogin =
                    authProfile.canLogin && authProfile.connectionState !== "connected";
                  const canLogout =
                    authProfile.canLogout && authProfile.connectionState === "connected";
                  return (
                    <div
                      key={authProfile.profile.id}
                      className="flex items-center justify-between gap-3 border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-2"
                    >
                      <div className="min-w-0">
                        <div className="truncate text-[12px] text-[var(--fg)]">
                          {authProfile.profile.displayName}
                        </div>
                        <div className="truncate text-[11px] text-[var(--fg-dim)]">
                          {authProfile.profile.providerId} · {authProfile.managementMode} ·{" "}
                          {authProfile.connectionState}
                        </div>
                        {authMethods.length > 0 ? (
                          <div className="truncate text-[11px] text-[var(--fg-dim)]">
                            methods ·{" "}
                            {authMethods
                              .map((method) => `${method.displayName} (${method.managementMode})`)
                              .join(", ")}
                          </div>
                        ) : null}
                        {authProfile.action?.description ? (
                          <div className="truncate text-[11px] text-[var(--fg-dim)]">
                            {authProfile.action.description}
                          </div>
                        ) : null}
                        {authProfile.action?.command ? (
                          <div className="truncate text-[11px] text-[var(--fg)]">
                            command · <code>{authProfile.action.command}</code>
                          </div>
                        ) : null}
                        {setupSteps.length > 0 ? (
                          <div className="text-[11px] text-[var(--fg-dim)]">
                            setup · {setupSteps.join(" -> ")}
                          </div>
                        ) : null}
                        {authProfile.lastError ? (
                          <div className="truncate text-[11px] text-[var(--status-failed)]">
                            {authProfile.lastError}
                          </div>
                        ) : null}
                        {authProfile.platformOrgLinked === false ? (
                          <div className="mt-1 flex flex-wrap items-center gap-2 text-[11px] text-[var(--fg-dim)]">
                            <Badge className="text-[10px]" variant="outline">
                              Connected · ChatGPT subscription only · Platform org not linked
                            </Badge>
                            <a
                              className="text-[var(--accent)] underline-offset-2 hover:underline"
                              href="https://platform.openai.com/settings/organization"
                              rel="noreferrer"
                              target="_blank"
                            >
                              Set up Platform org
                            </a>
                          </div>
                        ) : null}
                      </div>
                      {canLogin || canLogout ? (
                        <Button
                          disabled={isAuthActionPending}
                          onClick={() =>
                            canLogin
                              ? onAuthLogin(authProfile.profile.id)
                              : onAuthLogout(authProfile.profile.id)
                          }
                          size="sm"
                          type="button"
                          variant={canLogin ? "secondary" : "outline"}
                        >
                          {canLogin ? "Login" : "Logout"}
                        </Button>
                      ) : null}
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          <div className="space-y-2">
            <div className="text-[10px] uppercase tracking-[0.18em] text-[var(--fg-mute)]">
              Runtime extensions
            </div>
            {(snapshot.runtimeExtensions ?? []).length === 0 ? (
              <div className="text-[12px] text-[var(--fg-dim)]">No runtime extensions.</div>
            ) : (
              <div className="grid gap-2">
                {(snapshot.runtimeExtensions ?? []).map((extension) => (
                  <label
                    key={extension.descriptor.id}
                    className="flex items-center justify-between gap-3 border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-2"
                  >
                    <div className="min-w-0">
                      <div className="truncate text-[12px] text-[var(--fg)]">
                        {extension.descriptor.displayName}
                      </div>
                      <div className="truncate text-[11px] text-[var(--fg-dim)]">
                        {extension.availability} · {extension.descriptor.description}
                      </div>
                    </div>
                    <input
                      checked={extension.enabled}
                      className="size-4"
                      disabled={isMutating}
                      onChange={(event) =>
                        onSetExtensionEnabled(extension.descriptor.id, event.currentTarget.checked)
                      }
                      type="checkbox"
                    />
                  </label>
                ))}
              </div>
            )}
          </div>

          {mutationErrorMessage ? (
            <div className="text-[11px] text-[var(--status-failed)]">
              mutation failed · {mutationErrorMessage}
            </div>
          ) : null}

          {manualBrowserUrl ? (
            <div className="space-y-2 border border-amber-400/40 bg-amber-500/10 px-2 py-2 text-[11px] text-amber-100">
              <div>Browser launch failed. Copy this URL to finish sign-in manually:</div>
              <AuthLoginUrlRow label="Manual login URL" url={manualBrowserUrl} />
            </div>
          ) : null}

          {secondaryAuthorizeUrl ? (
            <div className="space-y-2 border border-[var(--border)] bg-[var(--bg-raised)] px-2 py-2 text-[11px] text-[var(--fg-dim)]">
              <div>If the browser does not open, use this authorization URL:</div>
              <AuthLoginUrlRow label="Authorization URL" url={secondaryAuthorizeUrl} />
            </div>
          ) : null}

          {errorMessage ? (
            <div className="text-[11px] text-[var(--status-failed)]">stale · {errorMessage}</div>
          ) : null}
        </div>
      )}
    </section>
  );
}

function AuthLoginUrlRow({ label, url }: { label: string; url: string }) {
  return (
    <div className="flex min-w-0 items-center gap-2">
      <a
        aria-label={label}
        className="min-w-0 flex-1 truncate font-[var(--font-mono)] underline"
        href={url}
        rel="noreferrer"
        target="_blank"
        title={url}
      >
        {url}
      </a>
      <Button
        onClick={() => {
          void navigator.clipboard.writeText(url);
        }}
        size="sm"
        type="button"
        variant="outline"
      >
        Copy
      </Button>
    </div>
  );
}

function ProviderHealthBanner({
  detail,
  label,
  tone,
}: {
  detail: string | null;
  label: string;
  tone: "ready" | "degraded" | "unavailable" | "unknown" | "missing";
}) {
  return (
    <div
      className={[
        "flex flex-wrap items-center gap-2 border px-2 py-1.5 text-[11px]",
        providerHealthBannerToneClass(tone),
      ].join(" ")}
      data-provider-health-banner={tone}
    >
      <span className="inline-flex items-center gap-1.5">
        <span
          aria-hidden="true"
          className={["inline-block h-1.5 w-1.5 rounded-full", providerHealthDotClass(tone)].join(
            " ",
          )}
        />
        <span className="uppercase tracking-[0.18em] text-[10px]">provider</span>
        <span className="font-[var(--font-mono)] text-[var(--fg)]">{label}</span>
      </span>
      {detail !== null ? (
        <span className="truncate text-[var(--fg-dim)]" title={detail}>
          {detail}
        </span>
      ) : null}
    </div>
  );
}

function providerHealthBannerToneClass(
  tone: "ready" | "degraded" | "unavailable" | "unknown" | "missing",
): string {
  switch (tone) {
    case "ready":
      return "border-[var(--border)] bg-[var(--bg-raised)] text-[var(--fg)]";
    case "degraded":
      return "border-amber-400/40 bg-amber-500/10 text-amber-200";
    case "unavailable":
      return "border-rose-400/40 bg-rose-500/10 text-rose-200";
    case "unknown":
      return "border-[var(--border)] bg-[var(--bg-sunken)] text-[var(--fg-dim)]";
    case "missing":
      return "border-dashed border-[var(--border)] bg-[var(--bg-sunken)] text-[var(--fg-mute)]";
  }
}

function providerHealthDotClass(
  tone: "ready" | "degraded" | "unavailable" | "unknown" | "missing",
): string {
  switch (tone) {
    case "ready":
      return "bg-emerald-500";
    case "degraded":
      return "bg-amber-400";
    case "unavailable":
      return "bg-rose-500";
    case "unknown":
      return "bg-slate-400";
    case "missing":
      return "bg-slate-500";
  }
}

function getSelectedProfile(
  snapshot: AgentRuntimeSnapshot | undefined,
): RuntimeProfileSummary | null {
  if (snapshot === undefined) {
    return null;
  }
  return (
    snapshot.runtimeProfiles?.find(
      (profile) => profile.id === snapshot.selection.runtimeProfileId,
    ) ?? null
  );
}

function providerById(
  providers: AgentRuntimeStrategyInfo[],
  providerId: AgentRuntimeStrategyId,
): AgentRuntimeStrategyInfo | null {
  return providers.find((provider) => provider.id === providerId) ?? null;
}

function authProfilesForProvider(
  authProfiles: AuthProfileState[],
  providerId: AgentRuntimeStrategyId | null,
): AuthProfileState[] {
  if (providerId === null) {
    return authProfiles;
  }
  return authProfiles.filter((authProfile) => authProfile.profile.providerId === providerId);
}

function describeModelCapability(
  provider: AgentRuntimeStrategyInfo,
  selectedModelId: AgentRuntimeModelId | null,
): string {
  const capability = provider.modelCapability;
  const modelCount = provider.models?.length ?? 0;
  switch (capability.availability) {
    case "enumerated": {
      if (selectedModelId !== null) {
        const selectedModel = provider.models?.find((model) => model.id === selectedModelId);
        const selectedLabel = selectedModel?.displayName ?? selectedModelId;
        return `selected ${selectedLabel}`;
      }
      return capability.currentModelId
        ? `provider default; current ${capability.currentModelId}`
        : `provider default; discovered ${modelCount} model(s)`;
    }
    case "currentOnly":
      return capability.currentModelId
        ? `current-only surface; active model ${capability.currentModelId}`
        : "current-only surface; provider does not enumerate models";
    case "unsupported":
      return capability.detail ?? "provider does not expose a selectable model catalog";
    case "unavailable":
      return capability.detail ?? "model discovery unavailable until provider is ready";
    case "unknown":
      return capability.detail ?? "model discovery state is unknown";
  }
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function firstErrorMessage(...errors: unknown[]): string | null {
  for (const error of errors) {
    if (error != null) {
      return toErrorMessage(error);
    }
  }
  return null;
}
