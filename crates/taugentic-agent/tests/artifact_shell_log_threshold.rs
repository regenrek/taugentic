#![cfg(target_os = "macos")]

mod support;

use std::fs;

use serde_json::json;
use support::*;
use ta_protocol::wire::{ArtifactKind, RunId};
use taugentic_agent::approval::ApprovalOutcome;
use taugentic_agent::artifacts::SHELL_LOG_ARTIFACT_THRESHOLD;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::tools::{Registry, ShellTool};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn artifact_shell_log_threshold_emits_only_for_large_or_truncated_output() {
    let small = run_shell("printf small", "run-shell-small").await;
    assert!(
        small.is_empty(),
        "small shell output should not emit artifact"
    );

    let large_command = format!(
        "i=0; while [ $i -lt {SHELL_LOG_ARTIFACT_THRESHOLD} ]; do printf x; i=$((i + 1)); done"
    );
    let large = run_shell(&large_command, "run-shell-large").await;
    assert_eq!(large.len(), 1);
    assert_eq!(large[0].0, ArtifactKind::CommandLog);
    assert!(large[0].1.ends_with(".log"));
}

async fn run_shell(command: &str, run_id: &str) -> Vec<(ArtifactKind, String)> {
    let temp = tempfile::tempdir().expect("tempdir");
    let workdir = temp.path().join("work");
    let artifact_root = temp.path().join("artifacts");
    fs::create_dir_all(&workdir).expect("workdir");
    let mut request = request();
    request.run_id = RunId::new(run_id).expect("run id");
    request.working_directory = workdir;
    request.artifact_root = artifact_root;

    let mut registry = Registry::new();
    let _ = registry.add(ShellTool);
    let client = MockClient::new(
        vec![MockStart::Stream(tool_turn_sequence(vec![(
            "shell".to_string(),
            json!({"command": command}).to_string(),
        )]))],
        false,
    );
    let sink = TestSink::new();
    let cancellation = CancellationToken::new();
    let context = LoopApprovalContext::new(request, sink.clone(), cancellation);
    let bridge = context.approval_bridge.clone();
    let mut loop_state = run_loop_with_bridge(client, registry, MessageQueue::default(), context);

    let task = tokio::spawn(async move { loop_state.execute().await });
    resolve_first_approval(&sink, &bridge, ApprovalOutcome::Allow).await;
    let result = task.await.expect("turn task should join");
    assert!(result.is_ok(), "{result:?}");
    sink.artifacts()
}
