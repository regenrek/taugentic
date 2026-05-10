use ta_protocol::wire::{
    AgentRuntimeModelId, AgentRuntimeModelRef, AgentRuntimeStrategyId, AuthProfileId,
    AuthProfileRef, LocalModelApiStandard, LocalModelAuthMode, LocalModelEndpointCapabilities,
    LocalModelEndpointConfig, RuntimePolicyMode, RuntimeProfileId, RuntimeProfileSummary,
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
        local_model_strategy(),
    ];
    let acp_registry = AcpProviderRegistry::new(Vec::<AcpProviderSpec>::new())
        .expect("built-in ACP descriptors are unique");
    strategies.extend(acp_registry.providers().into_iter().map(acp_strategy));
    strategies.extend(declarative_strategies());
    strategies
}

fn codex_app_server_strategy() -> RegisteredStrategy {
    use ta_provider_llm::families::codex_app_server::{
        CODEX_API_KEY_AUTH_PROFILE_ID, CODEX_CHATGPT_AUTH_PROFILE_ID, CODEX_PROVIDER_ID,
    };

    let provider_id = strategy_id(CODEX_PROVIDER_ID);
    registered_strategy(
        strategy_descriptor(
            provider_id.clone(),
            "Codex",
            ta_provider_llm::catalog::codex_models(),
            vec![
                auth_profile(CODEX_CHATGPT_AUTH_PROFILE_ID, &provider_id, "Codex ChatGPT"),
                auth_profile(CODEX_API_KEY_AUTH_PROFILE_ID, &provider_id, "Codex API Key"),
            ],
            ta_provider_llm::catalog::codex_default_runtime_profiles(),
        ),
        StrategyKind::CodexAppServer,
    )
}

fn openai_strategy() -> RegisteredStrategy {
    use ta_provider_llm::families::openai::OPENAI_PROVIDER_ID;

    let provider_id = strategy_id(OPENAI_PROVIDER_ID);
    registered_strategy(
        strategy_descriptor(
            provider_id.clone(),
            "OpenAI",
            ta_provider_llm::catalog::openai_models(),
            ta_provider_llm::auth::openai::auth_profile_refs(),
            ta_provider_llm::catalog::openai_default_runtime_profiles(),
        ),
        StrategyKind::OpenAiNative,
    )
}

fn anthropic_strategy() -> RegisteredStrategy {
    use ta_provider_llm::families::anthropic::{
        ANTHROPIC_API_KEY_AUTH_PROFILE_ID, ANTHROPIC_API_KEY_ENV_VAR, ANTHROPIC_PROVIDER_ID,
    };

    let provider_id = strategy_id(ANTHROPIC_PROVIDER_ID);
    registered_strategy(
        strategy_descriptor(
            provider_id.clone(),
            "Anthropic",
            ta_provider_llm::catalog::anthropic_models(),
            vec![auth_profile(
                ANTHROPIC_API_KEY_AUTH_PROFILE_ID,
                &provider_id,
                "Anthropic API Key",
            )],
            ta_provider_llm::catalog::anthropic_default_runtime_profiles(),
        ),
        StrategyKind::AnthropicApiKey {
            env_var: ANTHROPIC_API_KEY_ENV_VAR,
        },
    )
}

pub(crate) const LOCAL_MODEL_PROVIDER_ID: &str = "local-model";

fn local_model_strategy() -> RegisteredStrategy {
    let provider_id = strategy_id(LOCAL_MODEL_PROVIDER_ID);
    registered_strategy(
        strategy_descriptor(
            provider_id.clone(),
            "Local Model Endpoint",
            Vec::new(),
            Vec::new(),
            local_model_runtime_profiles(provider_id),
        ),
        StrategyKind::LocalModelEndpoint,
    )
}

fn declarative_strategies() -> Vec<RegisteredStrategy> {
    ta_provider_llm::declarative::specs()
        .iter()
        .map(|spec| {
            let provider_id = strategy_id(spec.id.as_ref());
            let auth_profile_id = auth_profile_id(&format!("{}-api-key", spec.id.as_ref()));
            let descriptor = strategy_descriptor(
                provider_id.clone(),
                spec.display_name.as_ref(),
                spec.models
                    .iter()
                    .map(|model| AgentRuntimeModelRef {
                        id: model_id(model.id.as_ref()),
                        display_name: model.display_name.to_string(),
                        context_limit: model.context_limit,
                        input_token_cost_micros: model.input_token_cost_micros,
                        output_token_cost_micros: model.output_token_cost_micros,
                    })
                    .collect(),
                vec![AuthProfileRef {
                    id: auth_profile_id.clone(),
                    provider_id: provider_id.clone(),
                    display_name: format!("{} API Key", spec.display_name.as_ref()),
                }],
                declarative_runtime_profiles(spec, provider_id, auth_profile_id),
            );
            let env_var = ta_provider_llm::declarative::auth_env_var(spec);
            registered_strategy(descriptor, StrategyKind::OpenAiCompatible { env_var })
        })
        .collect()
}

fn local_model_runtime_profiles(provider_id: AgentRuntimeStrategyId) -> Vec<RuntimeProfileSummary> {
    [
        local_model_profile(
            "ollama",
            "Ollama",
            "http://127.0.0.1:11434/v1",
            LocalModelApiStandard::OllamaOpenAi,
            "gpt-oss:20b",
            local_capabilities(true, true, true, true, true),
        ),
        local_model_profile(
            "lm-studio",
            "LM Studio",
            "http://127.0.0.1:1234/v1",
            LocalModelApiStandard::LmStudioOpenAi,
            "model-identifier",
            local_capabilities(true, true, true, true, true),
        ),
        local_model_profile(
            "llama-cpp",
            "llama.cpp",
            "http://127.0.0.1:8080/v1",
            LocalModelApiStandard::LlamaCppOpenAi,
            "model",
            local_capabilities(true, true, true, true, true),
        ),
        local_model_profile(
            "vllm",
            "vLLM",
            "http://127.0.0.1:8000/v1",
            LocalModelApiStandard::VllmOpenAi,
            "model",
            local_capabilities(true, true, true, true, true),
        ),
        local_model_profile(
            "tgi",
            "TGI",
            "http://127.0.0.1:3000/v1",
            LocalModelApiStandard::TgiMessages,
            "tgi",
            local_capabilities(true, false, false, false, false),
        ),
        local_model_profile(
            "custom",
            "Custom OpenAI-Compatible",
            "http://127.0.0.1:8000/v1",
            LocalModelApiStandard::OpenAiChatCompletions,
            "model",
            local_capabilities(true, None, None, None, None),
        ),
    ]
    .into_iter()
    .map(|mut profile| {
        profile.provider_id = provider_id.clone();
        profile
    })
    .collect()
}

fn local_capabilities(
    streaming: bool,
    tools: impl Into<Option<bool>>,
    parallel_tool_calls: impl Into<Option<bool>>,
    responses_api: impl Into<Option<bool>>,
    vision: impl Into<Option<bool>>,
) -> LocalModelEndpointCapabilities {
    LocalModelEndpointCapabilities {
        streaming: Some(streaming),
        tools: tools.into(),
        parallel_tool_calls: parallel_tool_calls.into(),
        responses_api: responses_api.into(),
        vision: vision.into(),
    }
}

fn local_model_profile(
    suffix: &str,
    label: &str,
    base_url: &str,
    api_standard: LocalModelApiStandard,
    default_model: &str,
    capabilities: LocalModelEndpointCapabilities,
) -> RuntimeProfileSummary {
    let model_id = model_id(default_model);
    RuntimeProfileSummary {
        id: RuntimeProfileId::new(format!("runtime-local-{suffix}")).expect("runtime profile id"),
        display_name: format!("Local {label}"),
        provider_id: strategy_id(LOCAL_MODEL_PROVIDER_ID),
        model_id: Some(model_id.clone()),
        auth_profile_id: None,
        local_endpoint: Some(LocalModelEndpointConfig {
            base_url: base_url.to_string(),
            api_standard,
            auth_mode: LocalModelAuthMode::None,
            api_key_env: None,
            default_model: Some(model_id),
            model_discovery: true,
            capabilities: Some(capabilities),
        }),
        policy_mode: RuntimePolicyMode::RequireApproval,
    }
}

fn acp_strategy(provider: AcpProviderSpec) -> RegisteredStrategy {
    let provider_id = strategy_id(provider.provider_id());
    let descriptor = strategy_descriptor(
        provider_id.clone(),
        provider.display_name(),
        Vec::new(),
        Vec::new(),
        acp_profiles(&provider)
            .into_iter()
            .map(|(id, display_name, policy_mode)| RuntimeProfileSummary {
                id: RuntimeProfileId::new(id).expect("runtime profile id"),
                display_name,
                provider_id: provider_id.clone(),
                model_id: None,
                auth_profile_id: None,
                local_endpoint: None,
                policy_mode,
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

fn declarative_runtime_profiles(
    spec: &ta_provider_llm::declarative::DeclarativeProviderSpec,
    provider_id: AgentRuntimeStrategyId,
    auth_profile_id: AuthProfileId,
) -> Vec<RuntimeProfileSummary> {
    let safe_model = model_id(spec.default_model.as_ref());
    let fast_model = spec
        .fast_model
        .as_ref()
        .map(|model| model_id(model.as_ref()))
        .unwrap_or_else(|| safe_model.clone());
    [
        (
            "safe",
            "Safe",
            RuntimePolicyMode::RequireApproval,
            safe_model.clone(),
        ),
        ("allow", "Allow", RuntimePolicyMode::Allow, fast_model),
        ("deny", "Deny", RuntimePolicyMode::Deny, safe_model),
    ]
    .into_iter()
    .map(
        |(suffix, label, policy_mode, model_id)| RuntimeProfileSummary {
            id: RuntimeProfileId::new(format!("runtime-{}-{suffix}", spec.id.as_ref()))
                .expect("runtime profile id"),
            display_name: format!("{} {label}", spec.display_name.as_ref()),
            provider_id: provider_id.clone(),
            model_id: Some(model_id),
            auth_profile_id: Some(auth_profile_id.clone()),
            local_endpoint: None,
            policy_mode,
        },
    )
    .collect()
}

fn strategy_id(value: &str) -> AgentRuntimeStrategyId {
    AgentRuntimeStrategyId::new(value).expect("provider id")
}

fn model_id(value: &str) -> AgentRuntimeModelId {
    AgentRuntimeModelId::new(value).expect("model id")
}

fn auth_profile_id(value: &str) -> AuthProfileId {
    AuthProfileId::new(value).expect("auth profile id")
}

fn auth_profile(
    id: &str,
    provider_id: &AgentRuntimeStrategyId,
    display_name: &str,
) -> AuthProfileRef {
    AuthProfileRef {
        id: auth_profile_id(id),
        provider_id: provider_id.clone(),
        display_name: display_name.to_string(),
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
            "local-model",
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
