use ta_protocol::wire::{
    BoundedFileContent, METHOD_DAEMON_WORKSPACE_FILE_OPEN_EXTERNAL,
    METHOD_DAEMON_WORKSPACE_FILE_READ, METHOD_DAEMON_WORKSPACE_FILE_TREE,
    METHOD_DAEMON_WORKSPACE_FILE_WRITE, WorkspaceFileOpenExternalResult, WorkspaceFileReadResult,
    WorkspaceFileTreeResult, WorkspaceFileWriteResult, WorkspacePath,
};

use super::*;

#[test]
fn daemon_workspace_file_routes_enforce_project_scope_approval_and_revision() {
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
    let root = tempfile::tempdir().expect("workspace tempdir");
    std::fs::write(root.path().join("notes.txt"), "before\n").expect("notes should write");
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
    let base = serde_json::json!({
        "projectId": project_id,
        "workspaceId": workspace_id,
    });

    let tree_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(501),
            method: METHOD_DAEMON_WORKSPACE_FILE_TREE.to_string(),
            params: Some(base.clone()),
        },
    )
    .expect("file tree route should succeed");
    let tree: WorkspaceFileTreeResult =
        serde_json::from_value(tree_response).expect("tree response");
    assert!(tree.entries.iter().any(|entry| entry.path == "notes.txt"));

    let mut read_params = base.clone();
    read_params["path"] = serde_json::json!("notes.txt");
    let read_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(502),
            method: METHOD_DAEMON_WORKSPACE_FILE_READ.to_string(),
            params: Some(read_params.clone()),
        },
    )
    .expect("file read route should succeed");
    let read: WorkspaceFileReadResult =
        serde_json::from_value(read_response).expect("read response");
    let revision = read.content.revision().to_string();
    assert!(matches!(
        read.content,
        BoundedFileContent::Text { text, .. } if text == "before\n"
    ));

    let mut write_params = read_params.clone();
    write_params["expectedRevision"] = serde_json::json!(revision);
    write_params["text"] = serde_json::json!("after\n");
    write_params["userApproved"] = serde_json::json!(false);
    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(503),
            method: METHOD_DAEMON_WORKSPACE_FILE_WRITE.to_string(),
            params: Some(write_params.clone()),
        },
    )
    .expect_err("unapproved file write should fail");
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "WorkspaceFileWriteApprovalRequired"
        }))
    );

    write_params["userApproved"] = serde_json::json!(true);
    let write_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(504),
            method: METHOD_DAEMON_WORKSPACE_FILE_WRITE.to_string(),
            params: Some(write_params),
        },
    )
    .expect("approved file write should succeed");
    let write: WorkspaceFileWriteResult =
        serde_json::from_value(write_response).expect("write response");
    assert_eq!(write.path, "notes.txt");
    assert_eq!(
        std::fs::read_to_string(root.path().join("notes.txt")).expect("saved notes"),
        "after\n"
    );

    let external_response = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: crate::RequestId::Integer(505),
            method: METHOD_DAEMON_WORKSPACE_FILE_OPEN_EXTERNAL.to_string(),
            params: Some(read_params),
        },
    )
    .expect("external-open validation route should succeed");
    let external: WorkspaceFileOpenExternalResult =
        serde_json::from_value(external_response).expect("external-open response");
    assert_eq!(external.path.as_path(), root.path().join("notes.txt"));
}
