use std::sync::{Arc, Mutex};

#[cfg(test)]
use ta_protocol::wire::RuntimePolicyMode;
use ta_protocol::wire::{
    AgentRuntimeMediaCapabilities, AgentRuntimeSelection, AgentRuntimeSnapshot,
    AuthProfileConnectionState, AuthProfileId, AuthProfileLoginResult, AuthProfilePreferences,
    AuthProfileRef, AuthProfileState, DaemonAgentRuntimeAuthLoginCompleteParams,
    DaemonAgentRuntimeAuthLoginParams, DaemonAgentRuntimeAuthLogoutParams,
    DaemonAgentRuntimeAuthProfilePreferencesSetParams, DaemonAgentRuntimePatchProfileParams,
    DaemonAgentRuntimeSetExtensionEnabledParams, GetAgentRuntimeQuery, RuntimeExtensionState,
    RuntimeProfileExecutionKind, RuntimeProfileId, RuntimeProfileSummary,
};
use ta_store::{AuthProfileProjection, PersistenceStore};
use taugentic_agent::AgentExecutionHarness;
use thiserror::Error;
use uuid::Uuid;

use crate::orchestration::agent_runtime::{
    StrategyRegistry,
    auth_profiles::{complete_auth_profile_login, login_auth_profile, logout_auth_profile},
    config::{apply_runtime_profile_patch, validate_runtime_profile},
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

#[derive(Debug)]
pub(crate) struct AgentRuntimeService<S>
where
    S: PersistenceStore + Send,
{
    registry: StrategyRegistry,
    state: SharedAgentRuntimeState,
    store: Arc<Mutex<S>>,
}

impl<S> Clone for AgentRuntimeService<S>
where
    S: PersistenceStore + Send,
{
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            state: self.state.clone(),
            store: Arc::clone(&self.store),
        }
    }
}

#[derive(Debug, Clone)]
struct SharedAgentRuntimeState {
    inner: Arc<Mutex<AgentRuntimeState>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentRuntimeState {
    pub runtime_profiles: Vec<RuntimeProfileSummary>,
    pub runtime_extensions: Vec<RuntimeExtensionState>,
}

#[derive(Debug, Clone)]
pub(crate) struct ValidatedRunSelection {
    runtime_profile: RuntimeProfileSummary,
    route: ta_protocol::wire::RunExecutionRoute,
    execution_harness: AgentExecutionHarness,
    media_capabilities: AgentRuntimeMediaCapabilities,
}

impl ValidatedRunSelection {
    pub(crate) fn runtime_profile(&self) -> &RuntimeProfileSummary {
        &self.runtime_profile
    }

    pub(crate) fn route(&self) -> &ta_protocol::wire::RunExecutionRoute {
        &self.route
    }

    pub(crate) fn execution_harness(&self) -> &AgentExecutionHarness {
        &self.execution_harness
    }

    pub(crate) fn media_capabilities(&self) -> AgentRuntimeMediaCapabilities {
        self.media_capabilities.clone()
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AgentRuntimeServiceError {
    #[error(transparent)]
    Store(#[from] ta_store::StoreError),
    #[error("runtime profile does not exist: {0}")]
    RuntimeProfileNotFound(String),
    #[error("auth profile does not exist: {0}")]
    AuthProfileNotFound(String),
    #[error("auth method does not exist: {0}")]
    AuthMethodNotFound(String),
    #[error("auth profile is not connected: {0}")]
    AuthProfileNotConnected(String),
    #[error("runtime selection requires an auth profile")]
    MissingAuthProfile,
    #[error("runtime selection requires a model")]
    MissingModel,
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
    WorkspaceCapabilityUnsupported(ta_protocol::wire::WorkspaceCapabilityUnsupported),
    #[error("{0}")]
    ProviderExecutionFailed(String),
}

impl<S> AgentRuntimeService<S>
where
    S: PersistenceStore + Send,
{
    pub(crate) fn new(
        runtime: AgentRuntimeRuntime,
        registry: StrategyRegistry,
        store: Arc<Mutex<S>>,
    ) -> Self {
        let state = runtime.state.clone();
        Self {
            registry,
            state,
            store,
        }
    }

    pub(crate) fn snapshot(
        &self,
        _: &GetAgentRuntimeQuery,
    ) -> Result<AgentRuntimeSnapshot, AgentRuntimeServiceError> {
        let state = self.state.lock()?.clone();
        self.build_snapshot(&state)
    }

    #[cfg(test)]
    pub(crate) fn replace_model_catalog_for_tests(
        &self,
        catalog: ta_model_catalog::ModelCatalog,
    ) -> Result<(), AgentRuntimeServiceError> {
        self.registry.replace_catalog(catalog)
    }

    pub(crate) fn validate_run_selection(
        &self,
        selection: &AgentRuntimeSelection,
    ) -> Result<ValidatedRunSelection, AgentRuntimeServiceError> {
        let runtime_profile = {
            let state = self.state.lock()?;
            state
                .runtime_profiles
                .iter()
                .find(|profile| profile.id == selection.runtime_profile_id)
                .cloned()
                .ok_or_else(|| {
                    AgentRuntimeServiceError::RuntimeProfileNotFound(
                        selection.runtime_profile_id.as_str().to_string(),
                    )
                })?
        };
        let runtime_profile = validate_runtime_profile(&runtime_profile, &self.registry)?;
        self.validate_selection(selection, std::slice::from_ref(&runtime_profile))?;
        let execution_harness = self
            .registry
            .execution_harness_for_runtime_profile(&runtime_profile)?;
        let model = self
            .registry
            .runtime_snapshot()?
            .providers
            .into_iter()
            .find(|provider| provider.id == runtime_profile.provider_id)
            .and_then(|provider| {
                provider
                    .models
                    .into_iter()
                    .find(|model| Some(&model.id) == selection.model_id.as_ref())
            })
            .ok_or_else(|| AgentRuntimeServiceError::UnknownModel {
                provider_id: runtime_profile.provider_id.as_str().to_string(),
                model_id: selection
                    .model_id
                    .as_ref()
                    .map_or_else(String::new, |value| value.as_str().to_string()),
            })?;
        let media_capabilities = model.media_capabilities;
        let route = ta_protocol::wire::RunExecutionRoute {
            runtime_profile_id: runtime_profile.id.clone(),
            provider_id: runtime_profile.provider_id.clone(),
            harness: match runtime_profile.execution_kind {
                RuntimeProfileExecutionKind::AgentRun => {
                    crate::orchestration::run_harness_kind(&execution_harness)
                }
                RuntimeProfileExecutionKind::RealtimeVoice => {
                    ta_protocol::wire::RunHarnessKind::RealtimeVoice
                }
            },
            model_id: selection.model_id.clone(),
            auth_profile_id: selection.auth_profile_id.clone(),
        };
        Ok(ValidatedRunSelection {
            runtime_profile,
            route,
            execution_harness,
            media_capabilities,
        })
    }

    /// Normal agent-run admission deliberately excludes the realtime lane
    /// before an agent `ExecutionRequest` can be constructed.
    pub(crate) fn validate_agent_run_selection(
        &self,
        selection: &AgentRuntimeSelection,
    ) -> Result<ValidatedRunSelection, AgentRuntimeServiceError> {
        let validated = self.validate_run_selection(selection)?;
        if validated.runtime_profile.execution_kind != RuntimeProfileExecutionKind::AgentRun {
            return Err(AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                "realtime voice profiles require the voice execution route".to_string(),
            ));
        }
        Ok(validated)
    }

    pub(crate) fn media_capabilities_for_route(
        &self,
        route: &ta_protocol::wire::RunExecutionRoute,
    ) -> Result<AgentRuntimeMediaCapabilities, AgentRuntimeServiceError> {
        let model_id = route
            .model_id
            .as_ref()
            .ok_or(AgentRuntimeServiceError::MissingModel)?;
        self.registry
            .runtime_snapshot()?
            .providers
            .into_iter()
            .find(|provider| provider.id == route.provider_id)
            .and_then(|provider| {
                provider
                    .models
                    .into_iter()
                    .find(|model| model.id == *model_id)
            })
            .map(|model| model.media_capabilities)
            .ok_or_else(|| AgentRuntimeServiceError::UnknownModel {
                provider_id: route.provider_id.as_str().to_string(),
                model_id: model_id.as_str().to_string(),
            })
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
        let method = self
            .registry
            .auth_method_ref(&params.auth_method_id)
            .cloned()
            .ok_or_else(|| {
                AgentRuntimeServiceError::AuthMethodNotFound(
                    params.auth_method_id.as_str().to_string(),
                )
            })?;
        let auth_profile_id =
            ta_protocol::wire::AuthProfileId::new(format!("profile-{}", Uuid::new_v4().simple()))
                .expect("generated auth profile id");
        let mut result =
            login_auth_profile(&self.registry, &params.auth_method_id, &auth_profile_id).await?;
        if result.auth_profile.profile.auth_method_id != method.id
            || result.auth_profile.profile.provider_id != method.provider_id
        {
            return Err(AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                "provider returned an auth profile for a different method".to_string(),
            ));
        }
        let mut store = self.store.lock().map_err(|_| {
            AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                "auth profile store should not be poisoned".to_string(),
            )
        })?;
        let group_len = store
            .auth_profiles()?
            .into_iter()
            .filter(|profile| {
                profile.profile.profile.provider_id == method.provider_id
                    && profile.profile.profile.auth_method_id == method.id
            })
            .count();
        let order = group_len as u32;
        let is_default = group_len == 0;
        result.auth_profile.preferences = ta_protocol::wire::AuthProfilePreferences {
            label: result.auth_profile.profile.display_name.clone(),
            order,
            is_default,
        };
        store.save_auth_profile(AuthProfileProjection {
            profile: result.auth_profile.clone(),
            external_account_id: None,
        })?;
        Ok(result)
    }

    pub(crate) fn replace_auth_profile_preferences(
        &self,
        params: &DaemonAgentRuntimeAuthProfilePreferencesSetParams,
    ) -> Result<AgentRuntimeSnapshot, AgentRuntimeServiceError> {
        validate_auth_profile_preferences(&params.preferences)?;
        let state = self.state.lock()?.clone();
        let mut store = self.store.lock().map_err(|_| {
            AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                "auth profile store should not be poisoned".to_string(),
            )
        })?;
        store.replace_auth_profile_preferences(
            &params.auth_profile_id,
            params.preferences.clone(),
        )?;
        drop(store);
        self.build_snapshot(&state)
    }

    pub(crate) async fn logout_auth_profile(
        &self,
        params: &DaemonAgentRuntimeAuthLogoutParams,
    ) -> Result<ta_protocol::wire::AuthProfileLogoutResult, AgentRuntimeServiceError> {
        let mut profile = {
            let store = self.store.lock().map_err(|_| {
                AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                    "auth profile store should not be poisoned".to_string(),
                )
            })?;
            store
                .auth_profile(&params.auth_profile_id)?
                .ok_or_else(|| {
                    AgentRuntimeServiceError::AuthProfileNotFound(
                        params.auth_profile_id.as_str().to_string(),
                    )
                })?
        };
        let result = logout_auth_profile(&self.registry, &profile.profile.profile).await?;
        profile.profile.connection_state = AuthProfileConnectionState::LoggedOut;
        profile.profile.can_logout = false;
        profile.profile.can_login = true;
        let mut store = self.store.lock().map_err(|_| {
            AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                "auth profile store should not be poisoned".to_string(),
            )
        })?;
        store.save_auth_profile(profile)?;
        Ok(result)
    }

    pub(crate) async fn complete_auth_profile_login(
        &self,
        params: &DaemonAgentRuntimeAuthLoginCompleteParams,
    ) -> Result<AuthProfileLoginResult, AgentRuntimeServiceError> {
        let mut projection = {
            let store = self.store.lock().map_err(|_| {
                AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                    "auth profile store should not be poisoned".to_string(),
                )
            })?;
            store
                .auth_profile(&params.auth_profile_id)?
                .ok_or_else(|| {
                    AgentRuntimeServiceError::AuthProfileNotFound(
                        params.auth_profile_id.as_str().to_string(),
                    )
                })?
        };
        if projection.profile.connection_state != AuthProfileConnectionState::PendingLogin {
            return Err(AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                "auth profile login is not pending".to_string(),
            ));
        }

        let result = complete_auth_profile_login(&self.registry, &projection.profile.profile).await;
        match result {
            Ok(result) => {
                if !same_auth_profile_identity(
                    &result.auth_profile.profile,
                    &projection.profile.profile,
                ) {
                    return Err(AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                        "provider completed a different auth profile".to_string(),
                    ));
                }
                projection.profile = result.auth_profile.clone();
                self.store
                    .lock()
                    .map_err(|_| {
                        AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                            "auth profile store should not be poisoned".to_string(),
                        )
                    })?
                    .save_auth_profile(projection)?;
                Ok(result)
            }
            Err(error) => {
                projection.profile.connection_state = AuthProfileConnectionState::Error;
                projection.profile.last_error =
                    Some("Authentication did not complete.".to_string());
                projection.profile.can_login = true;
                projection.profile.can_logout = false;
                self.store
                    .lock()
                    .map_err(|_| {
                        AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                            "auth profile store should not be poisoned".to_string(),
                        )
                    })?
                    .save_auth_profile(projection)?;
                Err(error)
            }
        }
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
            .map(|profile| validate_runtime_profile(profile, &self.registry))
            .collect::<Result<Vec<_>, _>>()?;
        let mut auth_profiles: Vec<_> = self
            .store
            .lock()
            .map_err(|_| {
                AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                    "auth profile store should not be poisoned".to_string(),
                )
            })?
            .auth_profiles()?
            .into_iter()
            .map(|profile| profile.profile)
            .collect();
        if snapshot_state.runtime_profiles.iter().any(|profile| {
            profile.execution_kind == RuntimeProfileExecutionKind::RealtimeVoice
                && profile.provider_id.as_str() == "openai"
                && profile
                    .auth_method_id
                    .as_ref()
                    .is_some_and(|method| method.as_str() == "openai-api-key")
        }) {
            auth_profiles.push(realtime_environment_auth_profile());
        }
        build_snapshot(
            &snapshot_state,
            runtime.providers,
            runtime.auth_methods,
            auth_profiles,
        )
    }

    fn validate_selection(
        &self,
        selection: &AgentRuntimeSelection,
        profiles: &[RuntimeProfileSummary],
    ) -> Result<(), AgentRuntimeServiceError> {
        let runtime_profile = profiles
            .iter()
            .find(|profile| profile.id == selection.runtime_profile_id)
            .ok_or_else(|| {
                AgentRuntimeServiceError::RuntimeProfileNotFound(
                    selection.runtime_profile_id.as_str().to_string(),
                )
            })?;
        let model_id = selection
            .model_id
            .as_ref()
            .ok_or(AgentRuntimeServiceError::MissingModel)?;
        if !self
            .registry
            .has_model(&runtime_profile.provider_id, model_id)
        {
            return Err(AgentRuntimeServiceError::UnknownModel {
                provider_id: runtime_profile.provider_id.as_str().to_string(),
                model_id: model_id.as_str().to_string(),
            });
        }
        match (&runtime_profile.auth_method_id, &selection.auth_profile_id) {
            (None, None) => Ok(()),
            (Some(_), None) => Err(AgentRuntimeServiceError::MissingAuthProfile),
            (None, Some(_)) => Err(AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                "runtime profile does not accept an auth profile".to_string(),
            )),
            (Some(auth_method_id), Some(auth_profile_id)) => {
                if runtime_profile.execution_kind == RuntimeProfileExecutionKind::RealtimeVoice
                    && *auth_profile_id == realtime_environment_auth_profile_id()
                {
                    if auth_method_id.as_str() != "openai-api-key" {
                        return Err(AgentRuntimeServiceError::UnknownAuthProfile {
                            provider_id: runtime_profile.provider_id.as_str().to_string(),
                            auth_profile_id: auth_profile_id.as_str().to_string(),
                        });
                    }
                    return if ta_provider_llm::realtime::credentials_available() {
                        Ok(())
                    } else {
                        Err(AgentRuntimeServiceError::AuthProfileNotConnected(
                            auth_profile_id.as_str().to_string(),
                        ))
                    };
                }
                let profile = self
                    .store
                    .lock()
                    .map_err(|_| {
                        AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                            "auth profile store should not be poisoned".to_string(),
                        )
                    })?
                    .auth_profile(auth_profile_id)?
                    .ok_or_else(|| {
                        AgentRuntimeServiceError::AuthProfileNotFound(
                            auth_profile_id.as_str().to_string(),
                        )
                    })?;
                if profile.auth_method_id() != auth_method_id
                    || profile.profile.profile.provider_id != runtime_profile.provider_id
                {
                    return Err(AgentRuntimeServiceError::UnknownAuthProfile {
                        provider_id: runtime_profile.provider_id.as_str().to_string(),
                        auth_profile_id: auth_profile_id.as_str().to_string(),
                    });
                }
                if profile.profile.connection_state != AuthProfileConnectionState::Connected {
                    return Err(AgentRuntimeServiceError::AuthProfileNotConnected(
                        auth_profile_id.as_str().to_string(),
                    ));
                }
                Ok(())
            }
        }
    }
}

fn realtime_environment_auth_profile_id() -> AuthProfileId {
    AuthProfileId::new("profile-openai-api-key-environment").expect("environment profile id")
}

fn realtime_environment_auth_profile() -> AuthProfileState {
    let connected = ta_provider_llm::realtime::credentials_available();
    AuthProfileState {
        profile: AuthProfileRef {
            id: realtime_environment_auth_profile_id(),
            auth_method_id: ta_protocol::wire::AuthMethodId::new("openai-api-key")
                .expect("auth method id"),
            provider_id: ta_protocol::wire::AgentRuntimeStrategyId::new("openai")
                .expect("provider id"),
            display_name: "OpenAI API Key".to_string(),
            account_hint: None,
            plan_tier: None,
        },
        preferences: AuthProfilePreferences {
            label: "OpenAI API Key".to_string(),
            order: 0,
            is_default: true,
        },
        usage: ta_protocol::wire::AuthProfileUsage::Unavailable,
        connection_state: if connected {
            AuthProfileConnectionState::Connected
        } else {
            AuthProfileConnectionState::LoggedOut
        },
        exhaustion: None,
        last_error: None,
        management_mode: ta_protocol::wire::AuthProfileManagementMode::Environment,
        can_login: false,
        can_logout: false,
        platform_org_linked: None,
        setup_steps: if connected {
            Vec::new()
        } else {
            vec!["Set the OpenAI API key in the daemon environment.".to_string()]
        },
        action: None,
        methods: Vec::new(),
    }
}

fn same_auth_profile_identity(left: &AuthProfileRef, right: &AuthProfileRef) -> bool {
    left.id == right.id
        && left.auth_method_id == right.auth_method_id
        && left.provider_id == right.provider_id
}

fn validate_auth_profile_preferences(
    preferences: &AuthProfilePreferences,
) -> Result<(), AgentRuntimeServiceError> {
    let label = preferences.label.as_str();
    if label.is_empty()
        || label.trim() != label
        || label.chars().any(char::is_control)
        || label.chars().count() > 80
    {
        return Err(AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
            "auth profile label is invalid".to_string(),
        ));
    }
    Ok(())
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

fn default_agent_runtime_state(runtime_profiles: Vec<RuntimeProfileSummary>) -> AgentRuntimeState {
    AgentRuntimeState {
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
    use ta_protocol::wire::{AgentRuntimeStrategyId, AuthMethodId, AuthProfileId};

    fn auth_profile_ref(provider_id: &str) -> AuthProfileRef {
        AuthProfileRef {
            id: AuthProfileId::new("profile-test").expect("auth profile id"),
            auth_method_id: AuthMethodId::new("codex-chatgpt").expect("auth method id"),
            provider_id: AgentRuntimeStrategyId::new(provider_id).expect("provider id"),
            display_name: "Codex ChatGPT".to_string(),
            account_hint: None,
            plan_tier: None,
        }
    }

    #[test]
    fn auth_completion_accepts_provider_enriched_profile_metadata() {
        let pending = auth_profile_ref("codex");
        let mut completed = pending.clone();
        completed.account_hint = Some("person@example.test".to_string());
        completed.plan_tier = Some("pro".to_string());

        assert!(same_auth_profile_identity(&pending, &completed));
    }

    #[test]
    fn auth_completion_rejects_a_different_profile_owner() {
        let pending = auth_profile_ref("codex");
        let completed = auth_profile_ref("openai");

        assert!(!same_auth_profile_identity(&pending, &completed));
    }

    #[test]
    fn patching_selected_profile_updates_live_policy_mode() {
        let registry = StrategyRegistry::from_registered(built_in_agent_runtime_strategies())
            .expect("provider registry should initialize");
        let runtime = AgentRuntimeRuntime::new(built_in_runtime_profiles(&registry));
        let service = AgentRuntimeService::new(
            runtime.clone(),
            registry,
            Arc::new(Mutex::new(ta_store::InMemoryStore::current())),
        );

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
    }

    #[test]
    fn setting_extension_enabled_updates_snapshot() {
        let registry = StrategyRegistry::from_registered(built_in_agent_runtime_strategies())
            .expect("provider registry should initialize");
        let runtime = AgentRuntimeRuntime::new(built_in_runtime_profiles(&registry));
        let service = AgentRuntimeService::new(
            runtime,
            registry,
            Arc::new(Mutex::new(ta_store::InMemoryStore::current())),
        );

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
    fn snapshot_represents_an_empty_runtime_without_implicit_selection() {
        let registry =
            StrategyRegistry::new(Vec::new()).expect("empty provider registry should initialize");
        let runtime = AgentRuntimeRuntime::new(Vec::new());
        let service = AgentRuntimeService::new(
            runtime,
            registry,
            Arc::new(Mutex::new(ta_store::InMemoryStore::current())),
        );

        let snapshot = service
            .snapshot(&GetAgentRuntimeQuery {})
            .expect("empty runtime config should project");

        assert!(snapshot.providers.is_empty());
        assert!(snapshot.auth_methods.is_empty());
        assert!(snapshot.auth_profiles.is_empty());
        assert!(snapshot.runtime_profiles.is_empty());
    }
}
