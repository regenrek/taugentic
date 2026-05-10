use super::*;
use ta_protocol::wire::{
    LocalModelApiStandard, LocalModelAuthMode, LocalModelEndpointConfig, RuntimePolicyMode,
};
use ta_provider_acp::descriptor::{AcpLaunchKind, AcpProviderDescriptor};

fn strategy_id(value: &str) -> AgentRuntimeStrategyId {
    AgentRuntimeStrategyId::new(value).expect("provider id")
}

fn model_id(value: &str) -> AgentRuntimeModelId {
    AgentRuntimeModelId::new(value).expect("model id")
}

fn auth_profile_id(value: &str) -> AuthProfileId {
    AuthProfileId::new(value).expect("auth profile id")
}

fn profile_id(value: &str) -> RuntimeProfileId {
    RuntimeProfileId::new(value).expect("runtime profile id")
}

fn fake_strategy(provider: &str, runtime_profile: &str) -> StrategyDescriptor {
    let provider_id = strategy_id(provider);
    let model_id = model_id("model-a");
    let auth_id = auth_profile_id(&format!("auth-{provider}"));
    strategy_descriptor(
        provider_id.clone(),
        "Fake",
        vec![AgentRuntimeModelRef {
            id: model_id.clone(),
            display_name: "Model A".to_string(),
            context_limit: None,
            input_token_cost_micros: None,
            output_token_cost_micros: None,
        }],
        vec![AuthProfileRef {
            id: auth_id.clone(),
            provider_id: provider_id.clone(),
            display_name: "Auth A".to_string(),
        }],
        vec![RuntimeProfileSummary {
            id: profile_id(runtime_profile),
            display_name: "Runtime A".to_string(),
            provider_id,
            model_id: Some(model_id),
            auth_profile_id: Some(auth_id),
            local_endpoint: None,
            policy_mode: RuntimePolicyMode::Allow,
        }],
    )
}

fn fake_runtime_profile(provider: &str, runtime_profile: &str) -> RuntimeProfileSummary {
    RuntimeProfileSummary {
        id: profile_id(runtime_profile),
        display_name: "Runtime A".to_string(),
        provider_id: strategy_id(provider),
        model_id: Some(model_id("model-a")),
        auth_profile_id: Some(auth_profile_id(&format!("auth-{provider}"))),
        local_endpoint: None,
        policy_mode: RuntimePolicyMode::Allow,
    }
}

#[test]
fn rejects_duplicate_runtime_profile_ids_across_strategies() {
    let error = StrategyRegistry::new(vec![
        fake_strategy("first", "runtime-a"),
        fake_strategy("second", "runtime-a"),
    ])
    .expect_err("duplicate runtime profile should fail");

    assert!(error.to_string().contains("duplicate runtime profile"));
}

#[test]
fn validates_auth_profile_owner() {
    let mut strategy = fake_strategy("first", "runtime-a");
    strategy.auth_profiles[0].provider_id = strategy_id("other");

    let error = StrategyRegistry::new(vec![strategy]).expect_err("owner mismatch should fail");

    assert!(
        error
            .to_string()
            .contains("auth profile auth-first is owned")
    );
}

#[test]
fn resolves_runtime_profiles_and_auth_refs() {
    let registry =
        StrategyRegistry::new(vec![fake_strategy("first", "runtime-a")]).expect("registry");

    assert!(registry.contains_provider(&strategy_id("first")));
    assert!(registry.has_model(&strategy_id("first"), &model_id("model-a")));
    assert!(
        registry
            .auth_profile_ref(&auth_profile_id("auth-first"))
            .is_some()
    );
    assert_eq!(registry.default_runtime_profiles().len(), 1);
}

#[test]
fn derives_execution_harness_from_registered_strategy_kind() {
    let registry = StrategyRegistry::from_registered(vec![
        registered_strategy(
            fake_strategy("openai-native", "runtime-openai-native"),
            StrategyKind::OpenAiNative,
        ),
        registered_strategy(
            fake_strategy("anthropic-native", "runtime-anthropic-native"),
            StrategyKind::AnthropicApiKey {
                env_var: "ANTHROPIC_API_KEY",
            },
        ),
        registered_strategy(
            fake_strategy("compatible-native", "runtime-compatible-native"),
            StrategyKind::OpenAiCompatible { env_var: None },
        ),
        registered_strategy(
            fake_strategy("codex", "runtime-codex"),
            StrategyKind::CodexAppServer,
        ),
        registered_strategy(
            fake_strategy("codex-acp", "runtime-codex-acp"),
            StrategyKind::AcpChildProcess {
                provider: AcpProviderSpec::from_builtin(AcpLaunchKind::Codex),
            },
        ),
    ])
    .expect("registry");

    assert_eq!(
        registry
            .execution_harness_for_runtime_profile(&fake_runtime_profile(
                "openai-native",
                "runtime-openai-native"
            ))
            .expect("openai native harness"),
        AgentExecutionHarness::NativeLoop
    );
    assert_eq!(
        registry
            .execution_harness_for_runtime_profile(&fake_runtime_profile(
                "anthropic-native",
                "runtime-anthropic-native"
            ))
            .expect("anthropic native harness"),
        AgentExecutionHarness::NativeLoop
    );
    let compatible_harness = registry
        .execution_harness_for_runtime_profile(&fake_runtime_profile(
            "compatible-native",
            "runtime-compatible-native",
        ))
        .expect("compatible native harness");
    assert_eq!(compatible_harness, AgentExecutionHarness::NativeLoop);
    assert!(compatible_harness.supports_native_capabilities());

    assert_eq!(
        registry
            .execution_harness_for_runtime_profile(&fake_runtime_profile("codex", "runtime-codex"))
            .expect("codex harness"),
        AgentExecutionHarness::CodexAppServer
    );
    let acp_harness = registry
        .execution_harness_for_runtime_profile(&fake_runtime_profile(
            "codex-acp",
            "runtime-codex-acp",
        ))
        .expect("acp harness");
    assert_eq!(
        acp_harness,
        AgentExecutionHarness::Acp {
            provider: AcpProviderSpec::from_builtin(AcpLaunchKind::Codex),
        }
    );
    assert!(acp_harness.requires_external_process_boundary());
}

#[test]
fn local_endpoint_profiles_accept_arbitrary_non_empty_models() {
    let provider_id = strategy_id("local-model");
    let registry = StrategyRegistry::from_registered(vec![registered_strategy(
        strategy_descriptor(
            provider_id.clone(),
            "Local Model",
            Vec::new(),
            Vec::new(),
            vec![RuntimeProfileSummary {
                id: profile_id("runtime-local-custom"),
                display_name: "Local Custom".to_string(),
                provider_id: provider_id.clone(),
                model_id: Some(model_id("arbitrary-local-model")),
                auth_profile_id: None,
                local_endpoint: Some(LocalModelEndpointConfig {
                    base_url: "http://127.0.0.1:8000/v1".to_string(),
                    api_standard: LocalModelApiStandard::OpenAiChatCompletions,
                    auth_mode: LocalModelAuthMode::None,
                    api_key_env: None,
                    default_model: Some(model_id("arbitrary-local-model")),
                    model_discovery: true,
                    capabilities: None,
                }),
                policy_mode: RuntimePolicyMode::Allow,
            }],
        ),
        StrategyKind::LocalModelEndpoint,
    )])
    .expect("registry");

    assert!(registry.has_model_for_profile(
        &registry.default_runtime_profiles()[0],
        &model_id("another-local-model")
    ));
}

#[test]
fn openai_health_copy_treats_subscription_oauth_as_runnable() {
    let strategy = registered_strategy(
        fake_strategy("openai-native", "runtime-openai-native"),
        StrategyKind::OpenAiNative,
    );
    let observed = openai_observed_state_for_snapshot(
        &strategy,
        ta_provider_llm::auth::openai::OpenAiAuthSnapshot {
            api_key_configured: false,
            chatgpt_configured: true,
            auth_profiles: Vec::new(),
        },
    );

    assert_eq!(
        observed.health.status,
        AgentRuntimeStrategyHealthStatus::Ready
    );
    let message = observed.health.message.as_deref().expect("health message");
    assert!(message.contains("ChatGPT subscription credentials are configured"));
    assert!(!message.contains("modeled in the auth surface"));
}

#[test]
fn rejects_runtime_profile_with_unknown_harness_provider() {
    let registry =
        StrategyRegistry::new(vec![fake_strategy("known", "runtime-known")]).expect("registry");
    let error = registry
        .execution_harness_for_runtime_profile(&fake_runtime_profile("missing", "runtime-a"))
        .expect_err("missing provider should fail");

    assert!(error.to_string().contains("unknown provider missing"));
}

#[test]
fn acp_snapshot_encodes_descriptor_owned_delegated_auth_and_runtime_model_discovery() {
    let binary_name = if cfg!(windows) { "cmd.exe" } else { "sh" };
    let provider = AcpProviderSpec::new(
        AcpProviderDescriptor::new(
            "test-acp",
            "Test ACP",
            "Test ACP",
            AcpLaunchKind::Codex,
            binary_name,
            "TAUGENTIC_TEST_ACP_BIN",
        )
        .expect("test ACP descriptor"),
    );
    let provider_id = strategy_id(provider.provider_id());
    let registry = StrategyRegistry::from_registered(vec![registered_strategy(
        strategy_descriptor(
            provider_id.clone(),
            provider.display_name(),
            Vec::new(),
            Vec::new(),
            vec![RuntimeProfileSummary {
                id: profile_id("runtime-test-acp-safe"),
                display_name: "Test ACP Safe".to_string(),
                provider_id,
                model_id: None,
                auth_profile_id: None,
                local_endpoint: None,
                policy_mode: RuntimePolicyMode::RequireApproval,
            }],
        ),
        StrategyKind::AcpChildProcess { provider },
    )])
    .expect("registry");

    let snapshot = registry.runtime_snapshot().expect("snapshot");

    assert!(snapshot.auth_profiles.is_empty());
    assert!(snapshot.providers[0].models.is_empty());
    let message = snapshot.providers[0]
        .health
        .message
        .as_deref()
        .expect("health message");
    assert!(message.contains("authentication is delegated to the vendor CLI"));
    assert!(message.contains("session model availability is validated on run"));
}
