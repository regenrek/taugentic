use ta_protocol::wire::{
    AgentRuntimeStrategyId, AuthMethodId, AuthMethodRef, AuthProfileManagementMode,
    RuntimePolicyMode, RuntimeProfileExecutionKind, RuntimeProfileId, RuntimeProfileSummary,
};
use ta_provider_acp::descriptor::{AcpProviderRegistry, AcpProviderSpec};

use crate::orchestration::agent_runtime::strategy_registry::{
    RegisteredStrategy, StrategyKind, registered_strategy, strategy_descriptor,
};

pub(crate) fn built_in_agent_runtime_strategies() -> Vec<RegisteredStrategy> {
    let mut strategies = vec![
        codex_app_server_strategy(),
        openai_strategy(),
        anthropic_strategy(),
    ];
    let acp_registry = AcpProviderRegistry::new(Vec::<AcpProviderSpec>::new())
        .expect("built-in ACP descriptors are unique");
    strategies.extend(acp_registry.providers().into_iter().map(acp_strategy));
    strategies.extend(declarative_strategies());
    strategies.push(deepseek_harness_strategy());
    strategies
}

fn codex_app_server_strategy() -> RegisteredStrategy {
    use ta_provider_llm::families::codex_app_server::CODEX_PROVIDER_ID;

    let provider_id = strategy_id(CODEX_PROVIDER_ID);
    let method_id = auth_method_id("codex-chatgpt");
    registered_strategy(
        strategy_descriptor(
            provider_id.clone(),
            "Codex",
            vec![auth_method(
                method_id.clone(),
                &provider_id,
                "Codex ChatGPT",
                AuthProfileManagementMode::Interactive,
                true,
            )],
            default_runtime_profiles(&provider_id, Some(method_id), "runtime-codex", "Codex"),
        ),
        StrategyKind::CodexAppServer,
    )
}

fn openai_strategy() -> RegisteredStrategy {
    use ta_provider_llm::families::openai::OPENAI_PROVIDER_ID;

    let provider_id = strategy_id(OPENAI_PROVIDER_ID);
    let method_id = auth_method_id("openai-chatgpt");
    let api_key_method_id = auth_method_id("openai-api-key");
    registered_strategy(
        strategy_descriptor(
            provider_id.clone(),
            "OpenAI",
            vec![
                auth_method(
                    method_id.clone(),
                    &provider_id,
                    "OpenAI ChatGPT",
                    AuthProfileManagementMode::Interactive,
                    true,
                ),
                auth_method(
                    api_key_method_id.clone(),
                    &provider_id,
                    "OpenAI API Key",
                    AuthProfileManagementMode::Environment,
                    false,
                ),
            ],
            {
                let mut profiles = default_runtime_profiles(
                    &provider_id,
                    Some(method_id),
                    "runtime-openai",
                    "OpenAI",
                );
                profiles.push(RuntimeProfileSummary {
                    id: RuntimeProfileId::new("runtime-openai-realtime")
                        .expect("runtime profile id"),
                    display_name: "OpenAI Realtime".to_string(),
                    provider_id: provider_id.clone(),
                    auth_method_id: Some(api_key_method_id),
                    policy_mode: RuntimePolicyMode::Deny,
                    execution_kind: RuntimeProfileExecutionKind::RealtimeVoice,
                });
                profiles
            },
        ),
        StrategyKind::OpenAiNative,
    )
}

fn anthropic_strategy() -> RegisteredStrategy {
    use ta_provider_llm::families::anthropic::{ANTHROPIC_API_KEY_ENV_VAR, ANTHROPIC_PROVIDER_ID};

    let provider_id = strategy_id(ANTHROPIC_PROVIDER_ID);
    registered_strategy(
        strategy_descriptor(
            provider_id.clone(),
            "Anthropic",
            vec![auth_method(
                auth_method_id("anthropic-api-key"),
                &provider_id,
                "Anthropic API Key",
                AuthProfileManagementMode::Environment,
                false,
            )],
            default_runtime_profiles(
                &provider_id,
                Some(auth_method_id("anthropic-api-key")),
                "runtime-anthropic",
                "Anthropic",
            ),
        ),
        StrategyKind::AnthropicApiKey {
            env_var: ANTHROPIC_API_KEY_ENV_VAR,
        },
    )
}

fn declarative_strategies() -> Vec<RegisteredStrategy> {
    ta_provider_llm::declarative::specs()
        .iter()
        .filter(|spec| spec.id.as_ref() != "deepseek")
        .map(|spec| {
            let provider_id = strategy_id(spec.id.as_ref());
            let auth_method_id = auth_method_id(&format!("{}-api-key", spec.id.as_ref()));
            let descriptor = strategy_descriptor(
                provider_id.clone(),
                spec.display_name.as_ref(),
                vec![AuthMethodRef {
                    id: auth_method_id.clone(),
                    provider_id: provider_id.clone(),
                    display_name: format!("{} API Key", spec.display_name.as_ref()),
                    management_mode: AuthProfileManagementMode::Environment,
                    supports_multiple_profiles: false,
                }],
                default_runtime_profiles(
                    &provider_id,
                    Some(auth_method_id),
                    &format!("runtime-{}", spec.id.as_ref()),
                    spec.display_name.as_ref(),
                ),
            );
            let env_var = ta_provider_llm::declarative::auth_env_var(spec);
            registered_strategy(descriptor, StrategyKind::OpenAiCompatible { env_var })
        })
        .collect()
}

fn deepseek_harness_strategy() -> RegisteredStrategy {
    let provider_id = strategy_id("deepseek");
    registered_strategy(
        strategy_descriptor(
            provider_id.clone(),
            "DeepSeek Harness",
            Vec::new(),
            default_runtime_profiles(&provider_id, None, "runtime-deepseek", "DeepSeek Harness"),
        ),
        StrategyKind::DeepSeekHarness,
    )
}

fn acp_strategy(provider: AcpProviderSpec) -> RegisteredStrategy {
    let provider_id = strategy_id(provider.provider_id());
    let descriptor = strategy_descriptor(
        provider_id.clone(),
        provider.display_name(),
        Vec::new(),
        acp_profiles(&provider)
            .into_iter()
            .map(|(id, display_name, policy_mode)| RuntimeProfileSummary {
                id: RuntimeProfileId::new(id).expect("runtime profile id"),
                display_name,
                provider_id: provider_id.clone(),
                auth_method_id: None,
                policy_mode,
                execution_kind: RuntimeProfileExecutionKind::AgentRun,
            })
            .collect(),
    );
    registered_strategy(descriptor, StrategyKind::AcpChildProcess { provider })
}

fn acp_profiles(provider: &AcpProviderSpec) -> [(String, String, RuntimePolicyMode); 3] {
    [
        (
            format!("runtime-{}-safe", provider.provider_id()),
            format!("{} Safe", provider.runtime_profile_label()),
            RuntimePolicyMode::RequireApproval,
        ),
        (
            format!("runtime-{}-allow", provider.provider_id()),
            format!("{} Allow", provider.runtime_profile_label()),
            RuntimePolicyMode::Allow,
        ),
        (
            format!("runtime-{}-chat", provider.provider_id()),
            format!("{} Chat", provider.runtime_profile_label()),
            RuntimePolicyMode::Deny,
        ),
    ]
}

fn default_runtime_profiles(
    provider_id: &AgentRuntimeStrategyId,
    auth_method_id: Option<AuthMethodId>,
    id_prefix: &str,
    display_prefix: &str,
) -> Vec<RuntimeProfileSummary> {
    [
        ("safe", "Safe", RuntimePolicyMode::RequireApproval),
        ("allow", "Allow", RuntimePolicyMode::Allow),
        ("deny", "Deny", RuntimePolicyMode::Deny),
    ]
    .into_iter()
    .map(|(suffix, label, policy_mode)| RuntimeProfileSummary {
        id: RuntimeProfileId::new(format!("{id_prefix}-{suffix}")).expect("runtime profile id"),
        display_name: format!("{display_prefix} {label}"),
        provider_id: provider_id.clone(),
        auth_method_id: auth_method_id.clone(),
        policy_mode,
        execution_kind: RuntimeProfileExecutionKind::AgentRun,
    })
    .collect()
}

fn strategy_id(value: &str) -> AgentRuntimeStrategyId {
    AgentRuntimeStrategyId::new(value).expect("provider id")
}

fn auth_method_id(value: &str) -> AuthMethodId {
    AuthMethodId::new(value).expect("auth method id")
}

fn auth_method(
    id: AuthMethodId,
    provider_id: &AgentRuntimeStrategyId,
    display_name: &str,
    management_mode: AuthProfileManagementMode,
    supports_multiple_profiles: bool,
) -> AuthMethodRef {
    AuthMethodRef {
        id,
        provider_id: provider_id.clone(),
        display_name: display_name.to_string(),
        management_mode,
        supports_multiple_profiles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_strategies_preserve_stable_ids() {
        let ids = built_in_agent_runtime_strategies()
            .into_iter()
            .map(|strategy| strategy.descriptor.id.as_str().to_string())
            .collect::<Vec<_>>();

        for expected in [
            "codex",
            "openai",
            "anthropic",
            "codex-acp",
            "claude-acp",
            "cursor",
            "opencode",
            "copilot-acp",
            "openrouter",
        ] {
            assert!(
                ids.iter().any(|id| id == expected),
                "missing provider id {expected}; got {ids:?}",
            );
        }
    }

    #[tokio::test]
    async fn built_in_strategies_can_initialize_inside_a_runtime() {
        let strategies = built_in_agent_runtime_strategies();

        assert!(
            strategies
                .iter()
                .any(|strategy| strategy.descriptor.id.as_str() == "cursor"),
            "expected ACP strategies to register without probing during construction"
        );
    }
}
