use std::collections::BTreeMap;
use ta_model_catalog::{ModelCatalog, ModelCatalogStore};
use ta_protocol::wire::{
    AgentRuntimeModelAvailability, AgentRuntimeModelCapability, AgentRuntimeModelId,
    AgentRuntimeModelRef, AgentRuntimeStrategyHealth, AgentRuntimeStrategyHealthStatus,
    AgentRuntimeStrategyId, AgentRuntimeStrategyInfo, AuthMethodId, AuthMethodRef, AuthProfileId,
    AuthProfileLoginResult, AuthProfileLogoutResult, AuthProfileRef, RuntimeProfileId,
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
    auth_method_strategy_ids: BTreeMap<AuthMethodId, AgentRuntimeStrategyId>,
    auth_method_refs: BTreeMap<AuthMethodId, AuthMethodRef>,
    runtime_profiles: Vec<RuntimeProfileSummary>,
    catalog: ModelCatalogStore,
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
    pub auth_methods: Vec<AuthMethodRef>,
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
    pub auth_methods: Vec<AuthMethodRef>,
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
        let catalog =
            ModelCatalogStore::embedded().map_err(|error| invalid_config(error.to_string()))?;
        let mut registered = Vec::new();
        let mut by_id = BTreeMap::new();
        let mut auth_method_strategy_ids = BTreeMap::new();
        let mut auth_method_refs = BTreeMap::new();
        let mut runtime_profiles = Vec::new();
        let mut runtime_profile_ids = BTreeMap::<RuntimeProfileId, AgentRuntimeStrategyId>::new();

        for strategy in strategies {
            let descriptor = &strategy.descriptor;
            let strategy_id = descriptor.id.clone();
            if by_id
                .insert(strategy_id.clone(), strategy.clone())
                .is_some()
            {
                return Err(invalid_config(format!(
                    "duplicate provider registered: {}",
                    strategy_id.as_str(),
                )));
            }

            for auth_method in &descriptor.auth_methods {
                if auth_method.provider_id != strategy_id {
                    return Err(invalid_config(format!(
                        "auth method {} is owned by provider {} but was registered by {}",
                        auth_method.id.as_str(),
                        auth_method.provider_id.as_str(),
                        strategy_id.as_str(),
                    )));
                }
                if auth_method_strategy_ids
                    .insert(auth_method.id.clone(), strategy_id.clone())
                    .is_some()
                {
                    return Err(invalid_config(format!(
                        "duplicate auth method registered: {}",
                        auth_method.id.as_str(),
                    )));
                }
                auth_method_refs.insert(auth_method.id.clone(), auth_method.clone());
            }

            for profile in &descriptor.default_runtime_profiles {
                validate_profile_descriptor(profile, descriptor)?;
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
            auth_method_strategy_ids,
            auth_method_refs,
            runtime_profiles,
            catalog,
        })
    }

    pub(crate) fn runtime_snapshot(
        &self,
    ) -> Result<StrategyRuntimeSnapshot, AgentRuntimeServiceError> {
        let catalog = self.catalog.snapshot();
        let mut providers = Vec::with_capacity(self.strategies.len());
        let mut auth_methods = Vec::new();

        for strategy in &self.strategies {
            let observed = observe_strategy(strategy, &catalog);
            providers.push(observed.provider);
            auth_methods.extend(strategy.descriptor.auth_methods.clone());
        }

        Ok(StrategyRuntimeSnapshot {
            providers,
            auth_methods,
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
                StrategyKind::AcpChildProcess { .. } => true,
                _ => self
                    .catalog
                    .snapshot()
                    .contains_model(provider_id, model_id),
            })
    }

    pub(crate) fn auth_method_ref(&self, auth_method_id: &AuthMethodId) -> Option<&AuthMethodRef> {
        self.auth_method_refs.get(auth_method_id)
    }

    pub(crate) fn replace_catalog(
        &self,
        catalog: ModelCatalog,
    ) -> Result<(), AgentRuntimeServiceError> {
        self.catalog
            .replace(catalog)
            .map_err(|error| invalid_config(error.to_string()))
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
        auth_method_id: &AuthMethodId,
        auth_profile_id: &AuthProfileId,
    ) -> Result<AuthProfileLoginResult, AgentRuntimeServiceError> {
        let strategy_id = self.strategy_id_for_auth_method(auth_method_id)?;
        let strategy = self.strategy_for_id(&strategy_id)?;
        match &strategy.kind {
            StrategyKind::CodexAppServer => {
                let auth_method_id = auth_method_id.clone();
                let auth_profile_id = auth_profile_id.clone();
                tokio::task::spawn_blocking(move || {
                    ta_provider_llm::families::codex_app_server::login(
                        &auth_method_id,
                        &auth_profile_id,
                    )
                })
                .await
                .map_err(|error| {
                    AgentRuntimeServiceError::ProviderExecutionFailed(format!(
                        "Codex ChatGPT login worker failed: {error}"
                    ))
                })?
                .map_err(|error| execution_error_from_llm_client(error.into()))
                .map_err(map_execution_error)
            }
            StrategyKind::OpenAiNative => {
                ta_provider_llm::auth::openai::login(auth_method_id, auth_profile_id)
                    .await
                    .map_err(execution_error_from_llm_client)
                    .map_err(map_execution_error)
            }
            StrategyKind::AnthropicApiKey { .. } | StrategyKind::OpenAiCompatible { .. } => {
                Err(AgentRuntimeServiceError::ProviderExecutionFailed(format!(
                    "{} does not support managed auth profiles",
                    strategy.descriptor.display_name
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
        profile: &AuthProfileRef,
    ) -> Result<AuthProfileLogoutResult, AgentRuntimeServiceError> {
        let strategy_id = self.strategy_id_for_auth_method(&profile.auth_method_id)?;
        let strategy = self.strategy_for_id(&strategy_id)?;
        match &strategy.kind {
            StrategyKind::CodexAppServer => {
                ta_provider_llm::families::codex_app_server::logout(&profile.id)
                    .map_err(|error| execution_error_from_llm_client(error.into()))
                    .map_err(map_execution_error)
            }
            StrategyKind::OpenAiNative => ta_provider_llm::auth::openai::logout(&profile.id)
                .await
                .map_err(execution_error_from_llm_client)
                .map_err(map_execution_error),
            StrategyKind::AnthropicApiKey { .. } | StrategyKind::OpenAiCompatible { .. } => {
                Ok(AuthProfileLogoutResult {
                    auth_profile_id: profile.id.clone(),
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

    pub(crate) async fn complete_login(
        &self,
        profile: &AuthProfileRef,
    ) -> Result<AuthProfileLoginResult, AgentRuntimeServiceError> {
        let strategy_id = self.strategy_id_for_auth_method(&profile.auth_method_id)?;
        let strategy = self.strategy_for_id(&strategy_id)?;
        match &strategy.kind {
            StrategyKind::CodexAppServer => {
                let auth_profile_id = profile.id.clone();
                tokio::task::spawn_blocking(move || {
                    ta_provider_llm::families::codex_app_server::complete_login(&auth_profile_id)
                })
                .await
                .map_err(|error| {
                    AgentRuntimeServiceError::ProviderExecutionFailed(format!(
                        "Codex ChatGPT login completion worker failed: {error}"
                    ))
                })?
                .map_err(|error| execution_error_from_llm_client(error.into()))
                .map_err(map_execution_error)
            }
            _ => Err(AgentRuntimeServiceError::ProviderExecutionFailed(format!(
                "{} does not expose a daemon-owned login completion phase",
                strategy.descriptor.display_name
            ))),
        }
    }

    fn strategy_id_for_auth_method(
        &self,
        auth_method_id: &AuthMethodId,
    ) -> Result<AgentRuntimeStrategyId, AgentRuntimeServiceError> {
        self.auth_method_strategy_ids
            .get(auth_method_id)
            .cloned()
            .ok_or_else(|| {
                AgentRuntimeServiceError::AuthMethodNotFound(auth_method_id.as_str().to_string())
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
}

fn validate_profile_descriptor(
    profile: &RuntimeProfileSummary,
    descriptor: &StrategyDescriptor,
) -> Result<(), AgentRuntimeServiceError> {
    if profile.provider_id != descriptor.id {
        return Err(invalid_config(format!(
            "runtime profile {} is owned by provider {} but was registered by {}",
            profile.id.as_str(),
            profile.provider_id.as_str(),
            descriptor.id.as_str(),
        )));
    }
    if let Some(auth_method_id) = profile.auth_method_id.as_ref()
        && !descriptor
            .auth_methods
            .iter()
            .any(|method| method.id == *auth_method_id)
    {
        return Err(invalid_config(format!(
            "runtime profile {} references unknown auth method {}",
            profile.id.as_str(),
            auth_method_id.as_str(),
        )));
    }
    Ok(())
}

fn observe_strategy(
    strategy: &RegisteredStrategy,
    catalog: &ModelCatalog,
) -> StrategyObservedState {
    match &strategy.kind {
        StrategyKind::CodexAppServer => observed_with_health(
            strategy,
            AgentRuntimeStrategyHealth {
                status: AgentRuntimeStrategyHealthStatus::Ready,
                message: None,
            },
            ta_provider_llm::families::codex_app_server::model_catalog()
                .map(|catalog| catalog.models)
                .unwrap_or_default(),
        ),
        StrategyKind::OpenAiNative => ready_strategy(strategy, catalog),
        StrategyKind::AnthropicApiKey { .. } | StrategyKind::OpenAiCompatible { .. } => {
            ready_strategy(strategy, catalog)
        }
        StrategyKind::AcpChildProcess { provider } => match ta_provider_acp::search_path::resolve(
            provider.binary_name(),
            provider.env_override_var(),
        ) {
            Ok(_) => observed_with_health(
                strategy,
                AgentRuntimeStrategyHealth {
                    status: AgentRuntimeStrategyHealthStatus::Ready,
                    message: None,
                },
                Vec::new(),
            ),
            Err(_) => observed_with_health(
                strategy,
                AgentRuntimeStrategyHealth {
                    status: AgentRuntimeStrategyHealthStatus::Unavailable,
                    message: Some("Provider integration is unavailable".to_string()),
                },
                Vec::new(),
            ),
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

fn ready_strategy(strategy: &RegisteredStrategy, catalog: &ModelCatalog) -> StrategyObservedState {
    observed_with_health(
        strategy,
        AgentRuntimeStrategyHealth {
            status: AgentRuntimeStrategyHealthStatus::Ready,
            message: None,
        },
        catalog.models(strategy.descriptor.id.as_str()),
    )
}

fn observed_with_health(
    strategy: &RegisteredStrategy,
    health: AgentRuntimeStrategyHealth,
    models: Vec<AgentRuntimeModelRef>,
) -> StrategyObservedState {
    StrategyObservedState {
        provider: AgentRuntimeStrategyInfo {
            id: strategy.descriptor.id.clone(),
            display_name: strategy.descriptor.display_name.clone(),
            models: models.clone(),
            model_capability: enumerated_model_capability(None, models.is_empty()),
            health,
        },
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

fn invalid_config(message: String) -> AgentRuntimeServiceError {
    AgentRuntimeServiceError::InvalidAgentRuntimeConfig(message)
}

fn map_execution_error(error: ExecutionError) -> AgentRuntimeServiceError {
    match error {
        ExecutionError::WorkspaceCapabilityUnsupported(detail) => {
            AgentRuntimeServiceError::WorkspaceCapabilityUnsupported(detail)
        }
        error => AgentRuntimeServiceError::ProviderExecutionFailed(error.to_string()),
    }
}

fn execution_error_from_llm_client(error: LlmClientError) -> ExecutionError {
    match error {
        LlmClientError::Auth(message) => ExecutionError::Auth(message),
        LlmClientError::CredentialsMissing(message) => ExecutionError::CredentialsMissing(message),
        LlmClientError::FeatureRequiresPlatformOrg => ExecutionError::FeatureRequiresPlatformOrg,
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
    auth_methods: Vec<AuthMethodRef>,
    default_runtime_profiles: Vec<RuntimeProfileSummary>,
) -> StrategyDescriptor {
    StrategyDescriptor {
        id,
        display_name: display_name.into(),
        auth_methods,
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
