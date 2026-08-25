mod support;

use std::fs;

use serde_json::json;
use support::*;
use ta_protocol::wire::{ArtifactKind, RunId};
use taugentic_agent::approval::ApprovalOutcome;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::tools::{ApplyPatchTool, Registry};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn artifact_patch_emission_persists_apply_patch_diff() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workdir = temp.path().join("work");
    let artifact_root = temp.path().join("artifacts");
    let artifact_root_for_assert = artifact_root.clone();
    fs::create_dir_all(&workdir).expect("workdir");
    let mut request = request_requiring_approval();
    request.run_id = RunId::new("run-artifact-patch").expect("run id");
    set_request_cwd(&mut request, &workdir);
    set_request_artifact_root(&mut request, &artifact_root);

    let patch = "*** Begin Patch\n*** Add File: hello.txt\n+hello\n*** End Patch\n";
    let mut registry = Registry::new();
    let _ = registry.add(ApplyPatchTool);
    let client = MockClient::new(
        vec![MockStart::Stream(tool_turn_sequence(vec![(
            "apply_patch".to_string(),
            json!({"input": patch}).to_string(),
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
    let artifacts = sink.artifacts();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].0, ArtifactKind::Patch);
    let artifact_path = std::path::PathBuf::from(&artifacts[0].1);
    assert_eq!(
        artifact_path.file_name().and_then(|name| name.to_str()),
        Some("patch-1.diff")
    );
    assert!(artifact_path.is_absolute());
    assert!(
        artifact_path.starts_with(
            artifact_root_for_assert
                .canonicalize()
                .expect("canonical artifact root")
        ),
        "recorded artifact path must be canonical and rooted"
    );
    assert!(
        fs::read_to_string(&artifact_path)
            .expect("diff artifact")
            .contains("hello.txt")
    );
}
