use std::sync::{Arc, Mutex};

use ta_protocol::wire::{
    AgentRuntimeSnapshot, AuthProfileLoginResult, DaemonAgentRuntimeAuthLoginParams,
    DaemonAgentRuntimeAuthLogoutParams, DaemonAgentRuntimePatchProfileParams,
    DaemonAgentRuntimeSelectProfileParams, DaemonAgentRuntimeSetExtensionEnabledParams,
    GetAgentRuntimeQuery, RuntimeExtensionState, RuntimePolicyMode, RuntimeProfileId,
    RuntimeProfileSummary,
};
use thiserror::Error;

use crate::orchestration::agent_runtime::{
    StrategyRegistry,
    auth_profiles::{login_auth_profile, logout_auth_profile},
    config::{apply_runtime_profile_patch, normalize_for_snapshot},
    extensions::{built_in_extensions, set_extension_enabled},
    snapshot::build_snapshot,
};

#[derive(Debug, Clone)]
pub(crate) struct AgentRuntimeRuntime {
    state: SharedAgentRuntimeState,
}

impl AgentRuntimeRuntime {
    pub(crate) fn new(runtime_profiles: Vec<RuntimeProfileSummary>) -> Self {
        Self {
            state: SharedAgentRuntimeState {
                inner: Arc::new(Mutex::new(default_agent_runtime_state(runtime_profiles))),
            },
        }
    }

    pub(crate) fn policy_mode(&self) -> Result<RuntimePolicyMode, AgentRuntimeServiceError> {
        self.state
            .lock()
            .expect("runtime state should not be poisoned")
            .selected_profile()
            .map(|profile| profile.policy_mode)
    }

    pub(crate) fn selected_profile(
        &self,
    ) -> Result<RuntimeProfileSummary, AgentRuntimeServiceError> {
        self.state
            .lock()
            .expect("runtime state should not be poisoned")
            .selected_profile()
            .cloned()
    }

    pub(crate) fn runtime_profile(
        &self,
        runtime_profile_id: &RuntimeProfileId,
    ) -> Option<RuntimeProfileSummary> {
        self.state
            .lock()
            .expect("runtime state should not be poisoned")
            .runtime_profiles
            .iter()
            .find(|profile| profile.id == *runtime_profile_id)
            .cloned()
    }

    pub(crate) fn runtime_extensions(&self) -> Vec<RuntimeExtensionState> {
        self.state
            .lock()
            .expect("runtime state should not be poisoned")
            .runtime_extensions
            .clone()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AgentRuntimeService {
    registry: StrategyRegistry,
    state: SharedAgentRuntimeState,
}

#[derive(Debug, Clone)]
struct SharedAgentRuntimeState {
    inner: Arc<Mutex<AgentRuntimeState>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentRuntimeState {
    pub selection: Option<RuntimeProfileId>,
    pub runtime_profiles: Vec<RuntimeProfileSummary>,
    pub runtime_extensions: Vec<RuntimeExtensionState>,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AgentRuntimeServiceError {
    #[error("no runtime provider configured")]
    NoRuntimeProviderConfigured,
    #[error("runtime profile does not exist: {0}")]
    RuntimeProfileNotFound(String),
    #[error("auth profile does not exist: {0}")]
    AuthProfileNotFound(String),
    #[error("runtime profile references unknown model {model_id} for provider {provider_id}")]
    UnknownModel {
        provider_id: String,
        model_id: String,
    },
    #[error(
        "runtime profile references unknown auth profile {auth_profile_id} for provider {provider_id}"
    )]
    UnknownAuthProfile {
        provider_id: String,
        auth_profile_id: String,
    },
    #[error("runtime extension does not exist: {0}")]
    RuntimeExtensionNotFound(String),
    #[error("{0}")]
    InvalidAgentRuntimeConfig(String),
    #[error("{0}")]
    ProviderExecutionFailed(String),
}

impl AgentRuntimeService {
    pub(crate) fn new(runtime: AgentRuntimeRuntime, registry: StrategyRegistry) -> Self {
        let state = runtime.state.clone();
        Self { registry, state }
    }

    pub(crate) fn snapshot(
        &self,
        _: &GetAgentRuntimeQuery,
    ) -> Result<AgentRuntimeSnapshot, AgentRuntimeServiceError> {
        let state = self.state.lock()?.clone();
        self.build_snapshot(&state)
    }

    pub(crate) fn select_profile(
        &self,
        params: &DaemonAgentRuntimeSelectProfileParams,
    ) -> Result<AgentRuntimeSnapshot, AgentRuntimeServiceError> {
        let mut state = self.state.lock()?;
        let selected_id = state
            .runtime_profiles
            .iter()
            .find(|profile| profile.id == params.runtime_profile_id)
            .map(|profile| profile.id.clone())
            .ok_or_else(|| {
                AgentRuntimeServiceError::RuntimeProfileNotFound(
                    params.runtime_profile_id.as_str().to_string(),
                )
            })?;
        state.selection = Some(selected_id);
        let snapshot_state = state.clone();
        drop(state);
        self.build_snapshot(&snapshot_state)
    }

    pub(crate) fn patch_profile(
        &self,
        params: &DaemonAgentRuntimePatchProfileParams,
    ) -> Result<AgentRuntimeSnapshot, AgentRuntimeServiceError> {
        let mut state = self.state.lock()?;
        apply_runtime_profile_patch(
            &mut state.runtime_profiles,
            &self.registry,
            &params.runtime_profile_id,
            &params.patch,
        )?;
        let snapshot_state = state.clone();
        drop(state);
        self.build_snapshot(&snapshot_state)
    }

    pub(crate) async fn login_auth_profile(
        &self,
        params: &DaemonAgentRuntimeAuthLoginParams,
    ) -> Result<AuthProfileLoginResult, AgentRuntimeServiceError> {
        login_auth_profile(&self.registry, &params.auth_profile_id).await
    }

    pub(crate) async fn logout_auth_profile(
        &self,
        params: &DaemonAgentRuntimeAuthLogoutParams,
    ) -> Result<ta_protocol::wire::AuthProfileLogoutResult, AgentRuntimeServiceError> {
        logout_auth_profile(&self.registry, &params.auth_profile_id).await
    }

    pub(crate) fn set_extension_enabled(
        &self,
        params: &DaemonAgentRuntimeSetExtensionEnabledParams,
    ) -> Result<AgentRuntimeSnapshot, AgentRuntimeServiceError> {
        let mut state = self.state.lock()?;
        set_extension_enabled(
            &mut state.runtime_extensions,
            &params.extension_id,
            params.enabled,
        )?;
        let snapshot_state = state.clone();
        drop(state);
        self.build_snapshot(&snapshot_state)
    }

    fn build_snapshot(
        &self,
        state: &AgentRuntimeState,
    ) -> Result<AgentRuntimeSnapshot, AgentRuntimeServiceError> {
        let runtime = self.registry.runtime_snapshot()?;
        let mut snapshot_state = state.clone();
        snapshot_state.runtime_profiles = state
            .runtime_profiles
            .iter()
            .map(|profile| normalize_for_snapshot(profile, &self.registry))
            .collect::<Result<Vec<_>, _>>()?;
        build_snapshot(&snapshot_state, runtime.providers, runtime.auth_profiles)
    }
}

impl SharedAgentRuntimeState {
    fn lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, AgentRuntimeState>, AgentRuntimeServiceError> {
        self.inner.lock().map_err(|_| {
            AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                "agent runtime state should not be poisoned".to_string(),
            )
        })
    }
}

impl AgentRuntimeState {
    fn selected_profile(&self) -> Result<&RuntimeProfileSummary, AgentRuntimeServiceError> {
        let selection = self
            .selection
            .as_ref()
            .ok_or(AgentRuntimeServiceError::NoRuntimeProviderConfigured)?;
        self.runtime_profiles
            .iter()
            .find(|profile| profile.id == *selection)
            .ok_or(AgentRuntimeServiceError::NoRuntimeProviderConfigured)
    }
}

fn default_agent_runtime_state(runtime_profiles: Vec<RuntimeProfileSummary>) -> AgentRuntimeState {
    let selection = runtime_profiles.first().map(|profile| profile.id.clone());
    AgentRuntimeState {
        selection,
        runtime_profiles,
        runtime_extensions: built_in_extensions(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::agent_runtime::{
        built_in_agent_runtime_strategies, built_in_runtime_profiles,
    };

    #[test]
    fn patching_selected_profile_updates_live_policy_mode() {
        let registry = StrategyRegistry::from_registered(built_in_agent_runtime_strategies())
            .expect("provider registry should initialize");
        let runtime = AgentRuntimeRuntime::new(built_in_runtime_profiles(&registry));
        let service = AgentRuntimeService::new(runtime.clone(), registry);

        let snapshot = service
            .patch_profile(&DaemonAgentRuntimePatchProfileParams {
                runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                patch: ta_protocol::wire::RuntimeProfilePatch {
                    policy_mode: Some(RuntimePolicyMode::Allow),
                    ..Default::default()
                },
            })
            .expect("runtime profile patch should succeed");
        let selected_profile = snapshot
            .runtime_profiles
            .iter()
            .find(|profile| profile.id.as_str() == "runtime-codex-safe")
            .expect("runtime profile should exist");

        assert_eq!(selected_profile.policy_mode, RuntimePolicyMode::Allow);
        assert_eq!(
            runtime.policy_mode().expect("runtime policy should exist"),
            RuntimePolicyMode::Allow
        );
    }

    #[test]
    fn setting_extension_enabled_updates_snapshot() {
        let registry = StrategyRegistry::from_registered(built_in_agent_runtime_strategies())
            .expect("provider registry should initialize");
        let runtime = AgentRuntimeRuntime::new(built_in_runtime_profiles(&registry));
        let service = AgentRuntimeService::new(runtime, registry);

        let snapshot = service
            .set_extension_enabled(&DaemonAgentRuntimeSetExtensionEnabledParams {
                extension_id: ta_protocol::wire::RuntimeExtensionId::new("local-shell-tools")
                    .expect("extension id"),
                enabled: false,
            })
            .expect("runtime extension toggle should succeed");
        let extension = snapshot
            .runtime_extensions
            .iter()
            .find(|extension| extension.descriptor.id.as_str() == "local-shell-tools")
            .expect("runtime extension should exist");

        assert!(!extension.enabled);
    }

    #[test]
    fn snapshot_fails_without_configured_runtime_provider() {
        let registry =
            StrategyRegistry::new(Vec::new()).expect("empty provider registry should initialize");
        let runtime = AgentRuntimeRuntime::new(Vec::new());
        let service = AgentRuntimeService::new(runtime, registry);

        let error = service
            .snapshot(&GetAgentRuntimeQuery {})
            .expect_err("empty runtime config should fail");

        assert!(matches!(
            error,
            AgentRuntimeServiceError::NoRuntimeProviderConfigured
        ));
    }
}
