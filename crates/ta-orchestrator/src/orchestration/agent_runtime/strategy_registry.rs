use std::collections::BTreeMap;
use std::env;

use ta_protocol::wire::{
    AgentRuntimeModelAvailability, AgentRuntimeModelCapability, AgentRuntimeModelId,
    AgentRuntimeModelRef, AgentRuntimeStrategyHealth, AgentRuntimeStrategyHealthStatus,
    AgentRuntimeStrategyId, AgentRuntimeStrategyInfo, AuthProfileConnectionState, AuthProfileId,
    AuthProfileLoginResult, AuthProfileLogoutResult, AuthProfileManagementMode,
    AuthProfileMethodInfo, AuthProfileRef, AuthProfileState, RuntimeProfileId,
    RuntimeProfileSummary,
};
use ta_provider_acp::descriptor::AcpProviderSpec;
use ta_provider_llm::error::LlmClientError;
use taugentic_agent::{AgentExecutionHarness, ExecutionError};

use crate::orchestration::AgentRuntimeServiceError;

#[derive(Clone)]
pub(crate) struct StrategyRegistry {
    strategies: Vec<RegisteredStrategy>,
    by_id: BTreeMap<AgentRuntimeStrategyId, RegisteredStrategy>,
    auth_profile_strategy_ids: BTreeMap<AuthProfileId, AgentRuntimeStrategyId>,
    auth_profile_refs: BTreeMap<AuthProfileId, AuthProfileRef>,
    runtime_profiles: Vec<RuntimeProfileSummary>,
}

#[derive(Clone)]
pub(crate) struct RegisteredStrategy {
    pub(crate) descriptor: StrategyDescriptor,
    kind: StrategyKind,
}

#[derive(Clone)]
pub(crate) struct StrategyDescriptor {
    pub id: AgentRuntimeStrategyId,
    pub display_name: String,
    pub models: Vec<AgentRuntimeModelRef>,
    pub auth_profiles: Vec<AuthProfileRef>,
    pub default_runtime_profiles: Vec<RuntimeProfileSummary>,
}

#[derive(Clone)]
pub(crate) enum StrategyKind {
    CodexAppServer,
    OpenAiNative,
    AnthropicApiKey { env_var: &'static str },
    OpenAiCompatible { env_var: Option<&'static str> },
    AcpChildProcess { provider: AcpProviderSpec },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StrategyRuntimeSnapshot {
    pub providers: Vec<AgentRuntimeStrategyInfo>,
    pub auth_profiles: Vec<AuthProfileState>,
}

impl std::fmt::Debug for StrategyRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrategyRegistry")
            .field("provider_ids", &self.by_id.keys().collect::<Vec<_>>())
            .field(
                "runtime_profile_ids",
                &self
                    .runtime_profiles
                    .iter()
                    .map(|profile| profile.id.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl StrategyRegistry {
    #[cfg(test)]
    pub(crate) fn new(
        strategies: Vec<StrategyDescriptor>,
    ) -> Result<Self, AgentRuntimeServiceError> {
        let strategies = strategies
            .into_iter()
            .map(|descriptor| RegisteredStrategy {
                kind: StrategyKind::OpenAiCompatible { env_var: None },
                descriptor,
            })
            .collect();
        Self::from_registered(strategies)
    }

    pub(crate) fn from_registered(
        strategies: Vec<RegisteredStrategy>,
    ) -> Result<Self, AgentRuntimeServiceError> {
        let mut registered = Vec::new();
        let mut by_id = BTreeMap::new();
        let mut auth_profile_strategy_ids = BTreeMap::new();
        let mut auth_profile_refs = BTreeMap::new();
        let mut runtime_profiles = Vec::new();
        let mut runtime_profile_ids = BTreeMap::<RuntimeProfileId, AgentRuntimeStrategyId>::new();

        for strategy in strategies {
            let descriptor = &strategy.descriptor;
            let strategy_id = descriptor.id.clone();
            let known_model_ids = descriptor
                .models
                .iter()
                .map(|model| model.id.clone())
                .collect::<Vec<_>>();

            if by_id
                .insert(strategy_id.clone(), strategy.clone())
                .is_some()
            {
                return Err(invalid_config(format!(
                    "duplicate provider registered: {}",
                    strategy_id.as_str(),
                )));
            }

            for auth_profile in &descriptor.auth_profiles {
                if auth_profile.provider_id != strategy_id {
                    return Err(invalid_config(format!(
                        "auth profile {} is owned by provider {} but was registered by {}",
                        auth_profile.id.as_str(),
                        auth_profile.provider_id.as_str(),
                        strategy_id.as_str(),
                    )));
                }
                if auth_profile_strategy_ids
                    .insert(auth_profile.id.clone(), strategy_id.clone())
                    .is_some()
                {
                    return Err(invalid_config(format!(
                        "duplicate auth profile registered: {}",
                        auth_profile.id.as_str(),
                    )));
                }
                auth_profile_refs.insert(auth_profile.id.clone(), auth_profile.clone());
            }

            for profile in &descriptor.default_runtime_profiles {
                validate_profile_descriptor(profile, descriptor, &known_model_ids)?;
                if runtime_profile_ids
                    .insert(profile.id.clone(), strategy_id.clone())
                    .is_some()
                {
                    return Err(invalid_config(format!(
                        "duplicate runtime profile registered: {}",
                        profile.id.as_str(),
                    )));
                }
                runtime_profiles.push(profile.clone());
            }

            registered.push(strategy);
        }

        Ok(Self {
            strategies: registered,
            by_id,
            auth_profile_strategy_ids,
            auth_profile_refs,
            runtime_profiles,
        })
    }

    pub(crate) fn runtime_snapshot(
        &self,
    ) -> Result<StrategyRuntimeSnapshot, AgentRuntimeServiceError> {
        let mut providers = Vec::with_capacity(self.strategies.len());
        let mut auth_profiles = Vec::new();

        for strategy in &self.strategies {
            let observed = observe_strategy(strategy);
            providers.push(observed.provider);
            auth_profiles.extend(observed.auth_profiles);
        }

        Ok(StrategyRuntimeSnapshot {
            providers,
            auth_profiles,
        })
    }

    pub(crate) fn default_runtime_profiles(&self) -> Vec<RuntimeProfileSummary> {
        self.runtime_profiles.clone()
    }

    pub(crate) fn contains_provider(&self, provider_id: &AgentRuntimeStrategyId) -> bool {
        self.by_id.contains_key(provider_id)
    }

    pub(crate) fn has_model(
        &self,
        provider_id: &AgentRuntimeStrategyId,
        model_id: &AgentRuntimeModelId,
    ) -> bool {
        self.by_id
            .get(provider_id)
            .is_some_and(|strategy| match &strategy.kind {
                StrategyKind::CodexAppServer => {
                    ta_provider_llm::families::codex_app_server::model_catalog().is_ok_and(
                        |catalog| catalog.models.iter().any(|model| model.id == *model_id),
                    )
                }
                _ => strategy
                    .descriptor
                    .models
                    .iter()
                    .any(|model| model.id == *model_id),
            })
    }

    pub(crate) fn auth_profile_ref(
        &self,
        auth_profile_id: &AuthProfileId,
    ) -> Option<&AuthProfileRef> {
        self.auth_profile_refs.get(auth_profile_id)
    }

    pub(crate) fn execution_harness_for_runtime_profile(
        &self,
        profile: &RuntimeProfileSummary,
    ) -> Result<AgentExecutionHarness, AgentRuntimeServiceError> {
        let strategy = self.by_id.get(&profile.provider_id).ok_or_else(|| {
            invalid_config(format!(
                "runtime profile {} references unknown provider {}",
                profile.id.as_str(),
                profile.provider_id.as_str(),
            ))
        })?;
        Ok(strategy.kind.execution_harness())
    }

    pub(crate) async fn login(
        &self,
        auth_profile_id: &AuthProfileId,
    ) -> Result<AuthProfileLoginResult, AgentRuntimeServiceError> {
        let strategy_id = self.strategy_id_for_auth_profile(auth_profile_id)?;
        let strategy = self.strategy_for_id(&strategy_id)?;
        match &strategy.kind {
            StrategyKind::CodexAppServer => {
                ta_provider_llm::families::codex_app_server::login(auth_profile_id)
                    .map_err(|error| execution_error_from_llm_client(error.into()))
                    .map_err(map_execution_error)
            }
            StrategyKind::OpenAiNative => ta_provider_llm::auth::openai::login(auth_profile_id)
                .await
                .map_err(execution_error_from_llm_client)
                .map_err(map_execution_error),
            StrategyKind::AnthropicApiKey { env_var }
            | StrategyKind::OpenAiCompatible {
                env_var: Some(env_var),
            } => login_env_auth(strategy, auth_profile_id, env_var),
            StrategyKind::OpenAiCompatible { env_var: None } => {
                Err(AgentRuntimeServiceError::ProviderExecutionFailed(format!(
                    "{} does not support interactive login for auth profile {}",
                    strategy.descriptor.display_name,
                    auth_profile_id.as_str()
                )))
            }
            StrategyKind::AcpChildProcess { .. } => {
                Err(AgentRuntimeServiceError::ProviderExecutionFailed(format!(
                    "{} delegates authentication to the vendor CLI",
                    strategy.descriptor.display_name
                )))
            }
        }
    }

    pub(crate) async fn logout(
        &self,
        auth_profile_id: &AuthProfileId,
    ) -> Result<AuthProfileLogoutResult, AgentRuntimeServiceError> {
        let strategy_id = self.strategy_id_for_auth_profile(auth_profile_id)?;
        let strategy = self.strategy_for_id(&strategy_id)?;
        match &strategy.kind {
            StrategyKind::CodexAppServer => {
                ta_provider_llm::families::codex_app_server::logout(auth_profile_id)
                    .map_err(|error| execution_error_from_llm_client(error.into()))
                    .map_err(map_execution_error)
            }
            StrategyKind::OpenAiNative => ta_provider_llm::auth::openai::logout(auth_profile_id)
                .await
                .map_err(execution_error_from_llm_client)
                .map_err(map_execution_error),
            StrategyKind::AnthropicApiKey { .. } | StrategyKind::OpenAiCompatible { .. } => {
                Ok(AuthProfileLogoutResult {
                    auth_profile_id: auth_profile_id.clone(),
                    disconnected: false,
                })
            }
            StrategyKind::AcpChildProcess { .. } => {
                Err(AgentRuntimeServiceError::ProviderExecutionFailed(format!(
                    "{} delegates logout to the vendor CLI",
                    strategy.descriptor.display_name
                )))
            }
        }
    }

    fn strategy_id_for_auth_profile(
        &self,
        auth_profile_id: &AuthProfileId,
    ) -> Result<AgentRuntimeStrategyId, AgentRuntimeServiceError> {
        self.auth_profile_strategy_ids
            .get(auth_profile_id)
            .cloned()
            .ok_or_else(|| {
                AgentRuntimeServiceError::AuthProfileNotFound(auth_profile_id.as_str().to_string())
            })
    }

    fn strategy_for_id(
        &self,
        provider_id: &AgentRuntimeStrategyId,
    ) -> Result<&RegisteredStrategy, AgentRuntimeServiceError> {
        self.by_id.get(provider_id).ok_or_else(|| {
            invalid_config(format!(
                "auth profile resolved to missing provider {}",
                provider_id.as_str(),
            ))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StrategyObservedState {
    provider: AgentRuntimeStrategyInfo,
    auth_profiles: Vec<AuthProfileState>,
}

fn validate_profile_descriptor(
    profile: &RuntimeProfileSummary,
    descriptor: &StrategyDescriptor,
    known_model_ids: &[AgentRuntimeModelId],
) -> Result<(), AgentRuntimeServiceError> {
    if profile.provider_id != descriptor.id {
        return Err(invalid_config(format!(
            "runtime profile {} is owned by provider {} but was registered by {}",
            profile.id.as_str(),
            profile.provider_id.as_str(),
            descriptor.id.as_str(),
        )));
    }
    if let Some(model_id) = profile.model_id.as_ref()
        && !known_model_ids
            .iter()
            .any(|known_model_id| known_model_id == model_id)
    {
        return Err(invalid_config(format!(
            "runtime profile {} references unknown model {} for provider {}",
            profile.id.as_str(),
            model_id.as_str(),
            descriptor.id.as_str(),
        )));
    }
    if let Some(auth_profile_id) = profile.auth_profile_id.as_ref()
        && !descriptor
            .auth_profiles
            .iter()
            .any(|profile| profile.id == *auth_profile_id)
    {
        return Err(invalid_config(format!(
            "runtime profile {} references unknown auth profile {}",
            profile.id.as_str(),
            auth_profile_id.as_str(),
        )));
    }
    Ok(())
}

fn observe_strategy(strategy: &RegisteredStrategy) -> StrategyObservedState {
    match &strategy.kind {
        StrategyKind::CodexAppServer => {
            match ta_provider_llm::families::codex_app_server::snapshot() {
                Ok(snapshot) => StrategyObservedState {
                    provider: snapshot.provider,
                    auth_profiles: snapshot.auth_profiles,
                },
                Err(error) => unavailable_with_profiles(strategy, error.to_string()),
            }
        }
        StrategyKind::OpenAiNative => openai_observed_state(strategy),
        StrategyKind::AnthropicApiKey { env_var }
        | StrategyKind::OpenAiCompatible {
            env_var: Some(env_var),
        } => env_observed_state(strategy, env_var),
        StrategyKind::OpenAiCompatible { env_var: None } => ready_with_profiles(strategy),
        StrategyKind::AcpChildProcess { provider } => match ta_provider_acp::search_path::resolve(
            provider.binary_name(),
            provider.env_override_var(),
        ) {
            Ok(path) => observed_with_health(
                strategy,
                AgentRuntimeStrategyHealth {
                    status: AgentRuntimeStrategyHealthStatus::Ready,
                    message: Some(format!(
                        "{} ACP adapter binary resolved at {}; authentication is delegated to the vendor CLI and session model availability is validated on run",
                        strategy.descriptor.display_name,
                        path.display()
                    )),
                },
                Vec::new(),
            ),
            Err(error) => unavailable_with_profiles(strategy, error.to_string()),
        },
    }
}

impl StrategyKind {
    fn execution_harness(&self) -> AgentExecutionHarness {
        match self {
            StrategyKind::CodexAppServer => AgentExecutionHarness::CodexAppServer,
            StrategyKind::OpenAiNative
            | StrategyKind::AnthropicApiKey { .. }
            | StrategyKind::OpenAiCompatible { .. } => AgentExecutionHarness::NativeLoop,
            StrategyKind::AcpChildProcess { provider } => AgentExecutionHarness::Acp {
                provider: provider.clone(),
            },
        }
    }
}

fn openai_observed_state(strategy: &RegisteredStrategy) -> StrategyObservedState {
    let snapshot = ta_provider_llm::auth::openai::snapshot();
    openai_observed_state_for_snapshot(strategy, snapshot)
}

fn openai_observed_state_for_snapshot(
    strategy: &RegisteredStrategy,
    snapshot: ta_provider_llm::auth::openai::OpenAiAuthSnapshot,
) -> StrategyObservedState {
    let ready = snapshot.api_key_configured || snapshot.chatgpt_configured;
    let api_key_env_var = ta_provider_llm::families::openai::OPENAI_API_KEY_ENV_VAR;
    observed_with_health(
        strategy,
        AgentRuntimeStrategyHealth {
            status: if ready {
                AgentRuntimeStrategyHealthStatus::Ready
            } else {
                AgentRuntimeStrategyHealthStatus::Degraded
            },
            message: Some(if ready {
                if snapshot.chatgpt_configured && !snapshot.api_key_configured {
                    "OpenAI ChatGPT subscription credentials are configured".to_string()
                } else if snapshot.chatgpt_configured {
                    "OpenAI API-key and ChatGPT subscription credentials are configured".to_string()
                } else {
                    "OpenAI API-key credentials are configured".to_string()
                }
            } else {
                format!(
                    "{} supports native API-key auth via {api_key_env_var} and browser login for OpenAI ChatGPT subscription auth",
                    strategy.descriptor.display_name
                )
            }),
        },
        snapshot.auth_profiles,
    )
}

fn env_observed_state(strategy: &RegisteredStrategy, env_var: &str) -> StrategyObservedState {
    let connected = env::var(env_var).is_ok_and(|value| !value.trim().is_empty());
    observed_with_health(
        strategy,
        AgentRuntimeStrategyHealth {
            status: if connected {
                AgentRuntimeStrategyHealthStatus::Ready
            } else {
                AgentRuntimeStrategyHealthStatus::Degraded
            },
            message: Some(if connected {
                format!(
                    "{} credentials are configured",
                    strategy.descriptor.display_name
                )
            } else {
                format!(
                    "{} requires {env_var} in the daemon environment",
                    strategy.descriptor.display_name
                )
            }),
        },
        strategy
            .descriptor
            .auth_profiles
            .iter()
            .cloned()
            .map(|profile| AuthProfileState {
                profile,
                connection_state: if connected {
                    AuthProfileConnectionState::Connected
                } else {
                    AuthProfileConnectionState::LoggedOut
                },
                last_error: None,
                management_mode: AuthProfileManagementMode::Environment,
                can_login: false,
                can_logout: false,
                platform_org_linked: None,
                setup_steps: vec![format!("Set {env_var} in the daemon environment")],
                action: None,
                methods: vec![AuthProfileMethodInfo {
                    id: "environment".to_string(),
                    display_name: "Environment".to_string(),
                    management_mode: AuthProfileManagementMode::Environment,
                }],
            })
            .collect(),
    )
}

fn ready_with_profiles(strategy: &RegisteredStrategy) -> StrategyObservedState {
    observed_with_health(
        strategy,
        AgentRuntimeStrategyHealth {
            status: AgentRuntimeStrategyHealthStatus::Ready,
            message: None,
        },
        strategy
            .descriptor
            .auth_profiles
            .iter()
            .cloned()
            .map(|profile| AuthProfileState {
                profile,
                connection_state: AuthProfileConnectionState::Connected,
                last_error: None,
                management_mode: AuthProfileManagementMode::Interactive,
                can_login: true,
                can_logout: true,
                platform_org_linked: None,
                setup_steps: Vec::new(),
                action: None,
                methods: vec![AuthProfileMethodInfo {
                    id: "interactive".to_string(),
                    display_name: "Interactive".to_string(),
                    management_mode: AuthProfileManagementMode::Interactive,
                }],
            })
            .collect(),
    )
}

fn unavailable_with_profiles(
    strategy: &RegisteredStrategy,
    message: String,
) -> StrategyObservedState {
    observed_with_health(
        strategy,
        AgentRuntimeStrategyHealth {
            status: AgentRuntimeStrategyHealthStatus::Unavailable,
            message: Some(message.clone()),
        },
        strategy
            .descriptor
            .auth_profiles
            .iter()
            .cloned()
            .map(|profile| AuthProfileState {
                profile,
                connection_state: AuthProfileConnectionState::Error,
                last_error: Some(message.clone()),
                management_mode: AuthProfileManagementMode::Unknown,
                can_login: false,
                can_logout: false,
                platform_org_linked: None,
                setup_steps: Vec::new(),
                action: None,
                methods: Vec::new(),
            })
            .collect(),
    )
}

fn observed_with_health(
    strategy: &RegisteredStrategy,
    health: AgentRuntimeStrategyHealth,
    auth_profiles: Vec<AuthProfileState>,
) -> StrategyObservedState {
    StrategyObservedState {
        provider: AgentRuntimeStrategyInfo {
            id: strategy.descriptor.id.clone(),
            display_name: strategy.descriptor.display_name.clone(),
            models: strategy.descriptor.models.clone(),
            model_capability: enumerated_model_capability(
                None,
                strategy.descriptor.models.is_empty(),
            ),
            health,
        },
        auth_profiles,
    }
}

fn enumerated_model_capability(
    detail: Option<String>,
    models_empty: bool,
) -> AgentRuntimeModelCapability {
    AgentRuntimeModelCapability {
        availability: if models_empty {
            AgentRuntimeModelAvailability::Unknown
        } else {
            AgentRuntimeModelAvailability::Enumerated
        },
        can_set_model: !models_empty,
        current_model_id: None,
        detail,
    }
}

fn login_env_auth(
    strategy: &RegisteredStrategy,
    auth_profile_id: &AuthProfileId,
    env_var: &str,
) -> Result<AuthProfileLoginResult, AgentRuntimeServiceError> {
    if !env::var(env_var).is_ok_and(|value| !value.trim().is_empty()) {
        return Err(map_execution_error(ExecutionError::CredentialsMissing(
            format!("{} requires {env_var}", strategy.descriptor.display_name),
        )));
    }
    let profile = strategy
        .descriptor
        .auth_profiles
        .iter()
        .find(|profile| profile.id == *auth_profile_id)
        .cloned()
        .ok_or_else(|| {
            AgentRuntimeServiceError::AuthProfileNotFound(auth_profile_id.as_str().to_string())
        })?;
    Ok(AuthProfileLoginResult {
        auth_profile: AuthProfileState {
            profile,
            connection_state: AuthProfileConnectionState::Connected,
            last_error: None,
            management_mode: AuthProfileManagementMode::Environment,
            can_login: false,
            can_logout: false,
            platform_org_linked: None,
            setup_steps: vec![format!("Set {env_var} in the daemon environment")],
            action: None,
            methods: vec![AuthProfileMethodInfo {
                id: "environment".to_string(),
                display_name: "Environment".to_string(),
                management_mode: AuthProfileManagementMode::Environment,
            }],
        },
        challenge: None,
    })
}

fn invalid_config(message: String) -> AgentRuntimeServiceError {
    AgentRuntimeServiceError::InvalidAgentRuntimeConfig(message)
}

fn map_execution_error(error: ExecutionError) -> AgentRuntimeServiceError {
    AgentRuntimeServiceError::ProviderExecutionFailed(error.to_string())
}

fn execution_error_from_llm_client(error: LlmClientError) -> ExecutionError {
    match error {
        LlmClientError::Auth(message) => ExecutionError::Auth(message),
        LlmClientError::CredentialsMissing(message) => ExecutionError::CredentialsMissing(message),
        LlmClientError::FeatureRequiresPlatformOrg => ExecutionError::FeatureRequiresPlatformOrg,
        LlmClientError::SubscriptionAuthIncompatibleWithNativeClient => {
            ExecutionError::SubscriptionAuthIncompatibleWithNativeClient
        }
        LlmClientError::Network(message) => ExecutionError::Network(message),
        LlmClientError::RateLimited {
            retry_after_ms,
            detail,
        } => ExecutionError::RateLimited {
            retry_after_ms,
            detail,
        },
        LlmClientError::CreditsExhausted(message) => ExecutionError::CreditsExhausted(message),
        LlmClientError::ContextLengthExceeded(message) => {
            ExecutionError::ContextLengthExceeded(message)
        }
        LlmClientError::InvalidConfig(message) => ExecutionError::InvalidConfig(message),
        LlmClientError::ProcessFailed(message) => ExecutionError::ProcessFailed(message),
        LlmClientError::Cancelled(message) => ExecutionError::Cancelled(message),
        LlmClientError::Unsupported(message) => ExecutionError::Unsupported(message),
        LlmClientError::ServerError(message) => ExecutionError::ServerError(message),
    }
}

pub(crate) fn strategy_descriptor(
    id: AgentRuntimeStrategyId,
    display_name: impl Into<String>,
    models: Vec<AgentRuntimeModelRef>,
    auth_profiles: Vec<AuthProfileRef>,
    default_runtime_profiles: Vec<RuntimeProfileSummary>,
) -> StrategyDescriptor {
    StrategyDescriptor {
        id,
        display_name: display_name.into(),
        models,
        auth_profiles,
        default_runtime_profiles,
    }
}

pub(crate) fn registered_strategy(
    descriptor: StrategyDescriptor,
    kind: StrategyKind,
) -> RegisteredStrategy {
    RegisteredStrategy { descriptor, kind }
}

#[cfg(test)]
#[path = "strategy_registry_tests.rs"]
mod tests;
