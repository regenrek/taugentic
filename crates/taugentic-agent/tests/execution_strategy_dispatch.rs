mod support;

use ta_provider_acp::descriptor::{AcpLaunchKind, AcpProviderSpec};
use taugentic_agent::{AgentExecutionHarness, AgentExecutionHarnessOwnership, run};

#[tokio::test]
async fn dispatch_reads_explicit_native_harness_instead_of_profile_string() {
    let mut request = support::request();
    request.runtime_profile_id =
        ta_protocol::wire::RuntimeProfileId::new("runtime-codex-api-key").expect("profile");
    request.provider_id =
        ta_protocol::wire::AgentRuntimeStrategyId::new("codex").expect("provider");
    request.execution_harness = AgentExecutionHarness::NativeLoop;
    let sink = support::TestSink::new();

    let error = match run(request, sink).await {
        Ok(_) => panic!("native harness should fail before spawning"),
        Err(error) => error,
    };

    assert!(
        error
            .to_string()
            .contains("native loop client is not configured for provider codex"),
        "unexpected error: {error}"
    );
}

#[test]
fn request_can_select_acp_harness_without_profile_id_heuristics() {
    let mut request = support::request();
    request.execution_harness = AgentExecutionHarness::Acp {
        provider: AcpProviderSpec::from_builtin(AcpLaunchKind::Codex),
    };

    assert_eq!(
        request.execution_harness,
        AgentExecutionHarness::Acp {
            provider: AcpProviderSpec::from_builtin(AcpLaunchKind::Codex),
        }
    );
}

#[test]
fn harness_ownership_kind_separates_native_core_from_external_lanes() {
    let native = AgentExecutionHarness::NativeLoop;
    let acp = AgentExecutionHarness::Acp {
        provider: AcpProviderSpec::from_builtin(AcpLaunchKind::Codex),
    };
    let codex_app_server = AgentExecutionHarness::CodexAppServer;

    assert_eq!(
        native.ownership_kind(),
        AgentExecutionHarnessOwnership::Native
    );
    assert!(native.is_native());
    assert!(native.supports_native_capabilities());
    assert!(!native.requires_external_process_boundary());

    for external in [acp, codex_app_server] {
        assert_eq!(
            external.ownership_kind(),
            AgentExecutionHarnessOwnership::ExternalIntegration
        );
        assert!(external.is_external());
        assert!(!external.supports_native_capabilities());
        assert!(external.requires_external_process_boundary());
    }
}
