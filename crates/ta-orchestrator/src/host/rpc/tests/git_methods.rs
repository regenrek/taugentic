use std::process::Command;

use ta_protocol::wire::{
    GitCheckpointListResult, GitDiffResult, GitMutationResult, GitRepositorySnapshotResult,
    METHOD_DAEMON_GIT_CHECKPOINT_LIST, METHOD_DAEMON_GIT_COMMIT, METHOD_DAEMON_GIT_DIFF,
    METHOD_DAEMON_GIT_SNAPSHOT, METHOD_DAEMON_GIT_STAGE, WorkspacePath,
};

use super::*;

fn git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git should execute");
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

#[test]
fn daemon_git_routes_use_strict_generated_params_and_typed_results() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session = test_session();
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    initialize_client(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        TEST_CLIENT_NAME,
    );
    let principal_id = session_state
        .lock()
        .expect("session state")
        .principal_id
        .clone()
        .expect("initialized principal");
    let root = tempfile::tempdir().expect("repository tempdir");
    git(root.path(), &["init", "--initial-branch=main"]);
    git(
        root.path(),
        &["config", "user.email", "rpc-git-test@example.invalid"],
    );
    git(
        root.path(),
        &["config", "user.name", "Taugentic RPC Git Test"],
    );
    std::fs::write(root.path().join("tracked.txt"), "base\n").expect("tracked file");
    git(root.path(), &["add", "--", "tracked.txt"]);
    git(root.path(), &["commit", "-m", "initial"]);
    let (project_id, snapshot) = state
        .app
        .open_project(
            &principal_id,
            WorkspacePath::canonicalize_existing(root.path()).expect("workspace path"),
            true,
        )
        .expect("project should open");
    let workspace_id = snapshot
        .projects
        .iter()
        .find(|project| project.id == project_id)
        .and_then(|project| project.workspace_ids.first())
        .cloned()
        .expect("project workspace");
    std::fs::write(root.path().join("tracked.txt"), "changed\n").expect("changed file");
    let base = serde_json::json!({
        "projectId": project_id,
        "workspaceId": workspace_id,
    });

    let mut invalid = base.clone();
    invalid["unexpected"] = serde_json::json!(true);
    handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(801),
            method: METHOD_DAEMON_GIT_SNAPSHOT.to_string(),
            params: Some(invalid),
        },
    )
    .expect_err("unknown Git params must be rejected");

    let status: GitRepositorySnapshotResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(802),
                method: METHOD_DAEMON_GIT_SNAPSHOT.to_string(),
                params: Some(base.clone()),
            },
        )
        .expect("Git snapshot route"),
    )
    .expect("Git snapshot response");
    assert_eq!(status.snapshot.files.len(), 1);

    let mut paths = base.clone();
    paths["paths"] = serde_json::json!(["tracked.txt"]);
    let staged: GitMutationResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(803),
                method: METHOD_DAEMON_GIT_STAGE.to_string(),
                params: Some(paths),
            },
        )
        .expect("Git stage route"),
    )
    .expect("Git stage response");
    assert!(staged.snapshot.files[0].staged.is_some());

    let mut diff = base.clone();
    diff["scope"] = serde_json::json!({ "kind": "staged" });
    let patch: GitDiffResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(804),
                method: METHOD_DAEMON_GIT_DIFF.to_string(),
                params: Some(diff),
            },
        )
        .expect("Git diff route"),
    )
    .expect("Git diff response");
    assert!(patch.patch.contains("+changed"));

    let checkpoints: GitCheckpointListResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(805),
                method: METHOD_DAEMON_GIT_CHECKPOINT_LIST.to_string(),
                params: Some(base.clone()),
            },
        )
        .expect("Git checkpoint list route"),
    )
    .expect("Git checkpoint list response");
    assert!(checkpoints.checkpoints.is_empty());

    let mut commit = base;
    commit["message"] = serde_json::json!("commit through RPC");
    let committed: GitMutationResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(806),
                method: METHOD_DAEMON_GIT_COMMIT.to_string(),
                params: Some(commit),
            },
        )
        .expect("Git commit route"),
    )
    .expect("Git commit response");
    assert!(committed.commit.is_some());
}
