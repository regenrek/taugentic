mod support;

use std::sync::Arc;
use std::time::Duration;

use ta_protocol::wire::{
    AgentRuntimeModelId, AgentRuntimeStrategyId, AgentStreamFrame, RuntimeProfileId,
};
use ta_provider_acp::descriptor::{AcpLaunchKind, AcpProviderSpec};
use taugentic_agent::run;

fn live_smoke_enabled() -> bool {
    std::env::var("TAUGENTIC_LIVE_SMOKE").ok().as_deref() == Some("1")
}

#[tokio::test]
#[ignore]
async fn live_smoke_openai_responses() {
    if !live_smoke_enabled() || std::env::var("OPENAI_API_KEY").is_err() {
        return;
    }
    run_native_tool_smoke("runtime-openai-safe", "gpt-5.6-sol").await;
}

#[tokio::test]
#[ignore]
async fn live_smoke_anthropic_messages() {
    if !live_smoke_enabled() || std::env::var("ANTHROPIC_API_KEY").is_err() {
        return;
    }
    run_native_tool_smoke("runtime-anthropic-safe", "claude-sonnet-4-5").await;
}

#[tokio::test]
#[ignore]
async fn live_smoke_openai_compatible() {
    if !live_smoke_enabled() {
        return;
    }
    if std::env::var("OPENROUTER_API_KEY").is_ok() {
        run_native_tool_smoke("runtime-openrouter-safe", "anthropic/claude-sonnet-4.5").await;
    } else if std::env::var("GROQ_API_KEY").is_ok() {
        run_native_tool_smoke("runtime-groq-safe", "openai/gpt-oss-120b").await;
    }
}

#[tokio::test]
#[ignore]
async fn live_smoke_codex_acp() {
    if !live_smoke_enabled() || which("codex-acp").is_none() {
        return;
    }
    let mut request = support::request();
    request.runtime_profile_id = runtime_profile_id("runtime-codex-acp-safe");
    request.provider_id = provider_id("codex-acp");
    request.execution_harness = taugentic_agent::AgentExecutionHarness::Acp {
        provider: AcpProviderSpec::from_builtin(AcpLaunchKind::Codex),
    };
    request.model_id = Some(model_id("gpt-5.6-sol"));
    request.objective = "Reply with exactly: live smoke ok".to_string();
    let sink = support::TestSink::new();
    let _handle = run(request, sink.clone())
        .await
        .expect("codex ACP smoke starts");
    wait_for_terminal(&sink);
    assert_no_failure(&sink);
    assert_completed(&sink);
}

#[tokio::test]
#[ignore]
async fn live_smoke_codex_app_server() {
    if !live_smoke_enabled() || which("codex").is_none() {
        return;
    }
    let mut request = support::request();
    request.runtime_profile_id = runtime_profile_id("runtime-codex-safe");
    request.provider_id = provider_id("codex");
    request.execution_harness = taugentic_agent::AgentExecutionHarness::CodexAppServer;
    request.auth_profile_id = None;
    request.model_id = Some(model_id("gpt-5.6-sol"));
    request.objective = "Reply with exactly: live smoke ok".to_string();
    let sink = support::TestSink::new();
    let _handle = run(request, sink.clone())
        .await
        .expect("codex app-server smoke starts");
    wait_for_terminal(&sink);
    assert_no_failure(&sink);
    assert_completed(&sink);
}

async fn run_native_tool_smoke(runtime_profile: &str, model: &str) {
    let mut request = support::request();
    request.runtime_profile_id = runtime_profile_id(runtime_profile);
    request.provider_id = provider_id(provider_for_native_runtime_profile(runtime_profile));
    request.model_id = Some(model_id(model));
    support::set_request_cwd(&mut request, &std::env::current_dir().expect("current dir"));
    request.objective = concat!(
        "Use the read_file tool to read Cargo.toml, then answer with exactly ",
        "'live smoke ok'. Do not answer until the tool call completes."
    )
    .to_string();
    let sink = support::TestSink::new();
    let _handle = run(request, sink.clone())
        .await
        .expect("native live smoke starts");
    wait_for_terminal(&sink);
    assert_no_failure(&sink);
    assert_completed(&sink);
    assert!(
        sink.stream_frames()
            .iter()
            .any(|emission| matches!(emission.frame, AgentStreamFrame::ToolCallCompleted { .. })),
        "expected ToolCallCompleted"
    );
    assert!(
        sink.stream_frames()
            .iter()
            .any(|emission| matches!(emission.frame, AgentStreamFrame::AssistantTurnCompleted)),
        "expected AssistantTurnCompleted"
    );
}

fn runtime_profile_id(value: &str) -> RuntimeProfileId {
    RuntimeProfileId::new(value).expect("runtime profile id")
}

fn provider_id(value: &str) -> AgentRuntimeStrategyId {
    AgentRuntimeStrategyId::new(value).expect("provider id")
}

fn model_id(value: &str) -> AgentRuntimeModelId {
    AgentRuntimeModelId::new(value).expect("model id")
}

fn provider_for_native_runtime_profile(runtime_profile: &str) -> &str {
    match runtime_profile {
        "runtime-openai-safe" => "openai",
        "runtime-anthropic-safe" => "anthropic",
        "runtime-openrouter-safe" => "openrouter",
        "runtime-groq-safe" => "groq",
        _ => panic!("unknown native smoke runtime profile {runtime_profile}"),
    }
}

fn wait_for_terminal(sink: &Arc<support::TestSink>) {
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    while std::time::Instant::now() < deadline {
        if !sink.completed.lock().expect("complete").is_empty()
            || !sink.failed.lock().expect("failed").is_empty()
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("live smoke did not complete within 120s");
}

fn assert_completed(sink: &Arc<support::TestSink>) {
    assert!(
        !sink.completed.lock().expect("complete").is_empty(),
        "expected completion"
    );
}

fn assert_no_failure(sink: &Arc<support::TestSink>) {
    let failed = sink.failed.lock().expect("failed");
    assert!(failed.is_empty(), "live smoke failed: {failed:?}");
}

fn which(binary: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}
