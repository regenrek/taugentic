mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ta_protocol::wire::{
    ApprovalActor, ApprovalDecision, ApprovalResolution, ApprovalResolutionReason, ApprovalScope,
    PermissionPolicy,
};
use ta_provider_acp::{
    adapter::{AcpProcessConfig, DEFAULT_CANCEL_GRACE},
    descriptor::{AcpLaunchKind, AcpProviderSpec},
    launch::build_perimeter_profile,
};
use taugentic_agent::execution_strategy::acp::dispatch_with_config;

#[test]
fn acp_permission_request_with_prompt_id_collision_resolves_through_execution_handle() {
    let dir = unique_dir("acp-strategy-approval");
    fs::create_dir_all(&dir).expect("dir");
    let marker = dir.join("selected-option.txt");
    let script = dir.join("mock-acp.py");
    fs::write(
        &script,
        format!(
            r#"#!/usr/bin/env python3
import json, pathlib, sys
marker = pathlib.Path({marker:?})
sys.stdin.readline()
print(json.dumps({{"jsonrpc":"2.0","id":1,"result":{{"protocolVersion":1,"agentCapabilities":{{}}}}}}), flush=True)
sys.stdin.readline()
print(json.dumps({{"jsonrpc":"2.0","id":2,"result":{{"sessionId":"s1"}}}}), flush=True)
sys.stdin.readline()
print(json.dumps({{"jsonrpc":"2.0","id":3,"method":"session/request_permission","params":{{"sessionId":"s1","toolCall":{{"toolCallId":"tool-approve","title":"run shell","kind":"execute"}},"options":[{{"optionId":"allow-once","name":"Allow once","kind":"allow_once"}},{{"optionId":"reject-once","name":"Reject once","kind":"reject_once"}}]}}}}), flush=True)
response = json.loads(sys.stdin.readline())
marker.write_text(response["result"]["optionId"])
print(json.dumps({{"jsonrpc":"2.0","id":3,"result":{{"stopReason":"end_turn"}}}}), flush=True)
"#,
            marker = marker.display().to_string()
        ),
    )
    .expect("script");
    let mut permissions = fs::metadata(&script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).expect("permissions");

    let mut request = support::request();
    support::configure_codex_acp_request(&mut request);
    support::set_request_cwd(&mut request, &dir);
    let mut execution_context = (*request.execution_context).clone();
    execution_context.permission_policy = PermissionPolicy::WorkspaceWriteWithApproval;
    request.execution_context = std::sync::Arc::new(execution_context);
    let sink = support::TestSink::new();
    let handle = dispatch_with_config(
        request,
        sink.clone(),
        AcpProcessConfig {
            flavor_id: "codex-acp".to_string(),
            command: script.clone(),
            sandbox_profile: test_perimeter_profile(&dir, &script),
            args: Vec::new(),
            env: Vec::new(),
            env_remove: Vec::new(),
            work_dir: dir,
            mcp_servers: Vec::new(),
            session_mode_id: None,
            session_model_id: None,
            cancel_grace: DEFAULT_CANCEL_GRACE,
        },
    )
    .expect("handle");

    wait_for(|| !sink.approval_requests().is_empty());
    let approval = sink
        .approval_requests()
        .into_iter()
        .next()
        .expect("approval request");
    assert_eq!(approval.scope, ApprovalScope::ProcessExec);
    assert_eq!(approval.reason, "ACP permission requested for run shell");
    let actor = ApprovalActor::new("test-user").expect("actor");
    let mut resolution = ApprovalResolution::new(
        approval.id.clone(),
        approval.run_id.clone(),
        ApprovalDecision::Approved,
        ApprovalResolutionReason::User,
        actor,
        Some("allow once".to_string()),
    );
    if let Some(tool_call_id) = approval.tool_call_id.clone() {
        resolution = resolution.with_tool_call_id(tool_call_id);
    }
    handle
        .resolve_approval(resolution)
        .expect("approval should resolve through handle");

    wait_for(|| !sink.completed.lock().expect("complete").is_empty());
    drop(handle);
    assert_eq!(fs::read_to_string(marker).expect("marker"), "allow-once");
}

fn test_perimeter_profile(
    work_dir: &std::path::Path,
    command: &std::path::Path,
) -> ta_exec::SandboxProfile {
    let provider = AcpProviderSpec::from_builtin(AcpLaunchKind::Cursor);
    build_perimeter_profile(
        &provider,
        &support::test_execution_context(work_dir),
        command,
    )
    .expect("test ACP perimeter profile")
}

fn unique_dir(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{suffix}"))
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(condition());
}
