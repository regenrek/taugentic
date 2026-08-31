mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{SystemTime, UNIX_EPOCH};

use ta_protocol::wire::{
    ApprovalActor, ApprovalDecision, ApprovalResolution, ApprovalResolutionReason, ApprovalScope,
};
use taugentic_agent::execution_strategy::deepseek_harness::dispatch_with_runtime;
use taugentic_agent::{AgentExecutionHarness, ExecutionHandle};

#[test]
fn deepseek_harness_forwards_one_resolved_approval_without_deadlocking() {
    let runtime = fixture("shell");
    let mut request = support::request();
    request.execution_harness = AgentExecutionHarness::DeepSeekHarness;
    request.model_id =
        Some(ta_protocol::wire::AgentRuntimeModelId::new("deepseek-v4-pro").expect("model"));
    request
        .dsh_tool_approval_manifest
        .insert("shell".to_string(), ApprovalScope::ProcessExec);
    let sink = support::TestSink::new();
    let handle = dispatch_with_runtime(request, sink.clone(), runtime).expect("dispatch");

    let approval = sink.wait_for_approval();
    let resolution = ApprovalResolution::new(
        approval.id.clone(),
        approval.run_id.clone(),
        ApprovalDecision::Approved,
        ApprovalResolutionReason::User,
        ApprovalActor::new("test-user").expect("actor"),
        None,
    )
    .with_tool_call_id(approval.tool_call_id.expect("tool id"));
    handle
        .resolve_approval(resolution.clone())
        .expect("first resolution");
    assert!(
        handle.resolve_approval(resolution).is_err(),
        "duplicate must fail closed"
    );
    sink.wait_for_completion();
    assert_eq!(sink.approval_requests().len(), 1);
    assert_eq!(sink.completed.lock().expect("complete").len(), 1);
    assert!(sink.failed.lock().expect("failed").is_empty());
    drop(handle);
}

#[test]
fn deepseek_harness_rejects_unknown_daemon_approval_id_without_a_bridge_outcome() {
    let runtime = fixture("shell");
    let mut request = support::request();
    request.execution_harness = AgentExecutionHarness::DeepSeekHarness;
    request.model_id =
        Some(ta_protocol::wire::AgentRuntimeModelId::new("deepseek-v4-pro").expect("model"));
    request
        .dsh_tool_approval_manifest
        .insert("shell".to_string(), ApprovalScope::ProcessExec);
    let sink = support::TestSink::new();
    let handle = dispatch_with_runtime(request, sink.clone(), runtime).expect("dispatch");

    let approval = sink.wait_for_approval();
    let unknown = ApprovalResolution::new(
        ta_protocol::wire::ApprovalId::new("approval-unknown").expect("unknown approval id"),
        approval.run_id.clone(),
        ApprovalDecision::Approved,
        ApprovalResolutionReason::User,
        ApprovalActor::new("test-user").expect("actor"),
        None,
    )
    .with_tool_call_id(approval.tool_call_id.clone().expect("tool id"));
    assert!(
        handle.resolve_approval(unknown).is_err(),
        "unknown daemon approval id must fail closed"
    );

    let valid = ApprovalResolution::new(
        approval.id.clone(),
        approval.run_id.clone(),
        ApprovalDecision::Approved,
        ApprovalResolutionReason::User,
        ApprovalActor::new("test-user").expect("actor"),
        None,
    )
    .with_tool_call_id(approval.tool_call_id.expect("tool id"));
    handle.resolve_approval(valid).expect("valid resolution");
    sink.wait_for_completion();
    assert_eq!(sink.completed.lock().expect("complete").len(), 1);
    assert!(sink.failed.lock().expect("failed").is_empty());
    drop(handle);
}

#[test]
fn deepseek_harness_rejects_late_resolution_after_terminal_correlation_removal() {
    let runtime = fixture("shell");
    let mut request = support::request();
    request.execution_harness = AgentExecutionHarness::DeepSeekHarness;
    request.model_id =
        Some(ta_protocol::wire::AgentRuntimeModelId::new("deepseek-v4-pro").expect("model"));
    request
        .dsh_tool_approval_manifest
        .insert("shell".to_string(), ApprovalScope::ProcessExec);
    let sink = support::TestSink::new();
    let handle = dispatch_with_runtime(request, sink.clone(), runtime).expect("dispatch");

    let approval = sink.wait_for_approval();
    let resolution = ApprovalResolution::new(
        approval.id.clone(),
        approval.run_id.clone(),
        ApprovalDecision::Rejected,
        ApprovalResolutionReason::User,
        ApprovalActor::new("test-user").expect("actor"),
        None,
    )
    .with_tool_call_id(approval.tool_call_id.expect("tool id"));
    handle
        .resolve_approval(resolution.clone())
        .expect("first resolution");
    sink.wait_for_completion();
    assert!(
        handle.resolve_approval(resolution).is_err(),
        "late resolution after terminal correlation removal must fail closed"
    );
    assert_eq!(sink.completed.lock().expect("complete").len(), 1);
    assert!(sink.failed.lock().expect("failed").is_empty());
    drop(handle);
}

#[test]
fn deepseek_harness_rejects_mismatched_resolution_without_consuming_correlation() {
    let runtime = fixture("shell");
    let mut request = support::request();
    request.execution_harness = AgentExecutionHarness::DeepSeekHarness;
    request.model_id =
        Some(ta_protocol::wire::AgentRuntimeModelId::new("deepseek-v4-pro").expect("model"));
    request
        .dsh_tool_approval_manifest
        .insert("shell".to_string(), ApprovalScope::ProcessExec);
    let sink = support::TestSink::new();
    let handle = dispatch_with_runtime(request, sink.clone(), runtime).expect("dispatch");

    let approval = sink.wait_for_approval();
    let mismatched = ApprovalResolution::new(
        ta_protocol::wire::ApprovalId::new(approval.id.as_str()).expect("approval id"),
        ta_protocol::wire::RunId::new("other-run").expect("run id"),
        ApprovalDecision::Approved,
        ApprovalResolutionReason::User,
        ApprovalActor::new("test-user").expect("actor"),
        None,
    )
    .with_tool_call_id(approval.tool_call_id.clone().expect("tool id"));
    assert!(handle.resolve_approval(mismatched).is_err());

    let valid = ApprovalResolution::new(
        approval.id.clone(),
        approval.run_id.clone(),
        ApprovalDecision::Rejected,
        ApprovalResolutionReason::User,
        ApprovalActor::new("test-user").expect("actor"),
        None,
    )
    .with_tool_call_id(approval.tool_call_id.expect("tool id"));
    handle.resolve_approval(valid).expect("valid resolution");
    sink.wait_for_completion();
    assert_eq!(sink.approval_requests().len(), 1);
    assert_eq!(sink.completed.lock().expect("complete").len(), 1);
    drop(handle);
}

#[test]
fn deepseek_harness_rejects_unknown_manifest_tool_and_completes_no_sink() {
    let runtime = fixture("unknown");
    let mut request = support::request();
    request.execution_harness = AgentExecutionHarness::DeepSeekHarness;
    request.model_id =
        Some(ta_protocol::wire::AgentRuntimeModelId::new("deepseek-v4-flash").expect("model"));
    let sink = support::TestSink::new();
    let handle = dispatch_with_runtime(request, sink.clone(), runtime).expect("dispatch");
    sink.wait_for_failure();
    assert!(sink.approval_requests().is_empty());
    assert!(sink.completed.lock().expect("complete").is_empty());
    drop(handle);
}

#[test]
fn deepseek_harness_drop_reaps_a_silent_child() {
    let marker = std::env::temp_dir().join(format!(
        "taugentic-dsh-drop-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    let runtime = stalled_fixture(&marker);
    let mut request = support::request();
    request.execution_harness = AgentExecutionHarness::DeepSeekHarness;
    request.model_id =
        Some(ta_protocol::wire::AgentRuntimeModelId::new("deepseek-v4-pro").expect("model"));
    let sink = support::TestSink::new();
    let handle = dispatch_with_runtime(request, sink.clone(), runtime).expect("dispatch");

    sink.wait_for_stream();
    drop(handle);
    sink.wait_for_failure();
    assert_process_reaped(&marker);
    assert!(sink.completed.lock().expect("complete").is_empty());
}

fn fixture(tool_name: &str) -> ta_provider_dsh::SealedRuntime {
    let path = std::env::temp_dir().join(format!(
        "taugentic-dsh-agent-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::write(&path, format!(r#"#!/bin/sh
while IFS= read -r line; do case "$line" in
*initialize*) echo '{{"event":"initialized","protocol":"taugentic-dsh-bridge/v1","runtime":"dsh-v0.1.1-rc.2"}}' ;;
*prompt*) echo '{{"event":"stream","turnId":"turn-1","itemId":"item-1","delta":"hello"}}'; echo '{{"event":"approval","approvalId":"bridge-1","callId":"tool-1","toolName":"{tool_name}"}}' ;;
*approval*) echo '{{"event":"snapshot","continuation":"v1:opaque"}}'; echo '{{"event":"completed"}}' ;;
*cancel*) echo '{{"event":"cancelled"}}' ;;
*shutdown*) echo '{{"event":"shutdown"}}'; exit 0 ;;
esac; done
"#)).expect("fixture");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("permissions");
    ta_provider_dsh::SealedRuntime::from_sealed_executable(path).expect("runtime")
}

fn stalled_fixture(marker: &std::path::Path) -> ta_provider_dsh::SealedRuntime {
    let path = std::env::temp_dir().join(format!(
        "taugentic-dsh-stalled-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::write(&path, format!(r#"#!/bin/sh
while IFS= read -r line; do case "$line" in
*initialize*) echo '{{"event":"initialized","protocol":"taugentic-dsh-bridge/v1","runtime":"dsh-v0.1.1-rc.2"}}' ;;
*prompt*) echo $$ > "{}"; echo '{{"event":"stream","turnId":"turn-1","itemId":"item-1","delta":"started"}}'; while :; do :; done ;;
esac; done
"#, marker.display())).expect("fixture");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("permissions");
    ta_provider_dsh::SealedRuntime::from_sealed_executable(path).expect("runtime")
}

fn assert_process_reaped(marker: &std::path::Path) {
    let pid = fs::read_to_string(marker).expect("fixture pid marker");
    let status = std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .status()
        .expect("kill probe");
    assert!(!status.success(), "fixture child must be reaped");
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos()
}
