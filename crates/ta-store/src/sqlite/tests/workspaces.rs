use super::*;
use crate::{
    CommitSessionOpen, SessionProjection, StoreError, WorkspaceRepository, default_test_workspace,
    default_test_workspace_id, test_workspace,
};
use ta_protocol::wire::{SessionId, SessionStatus, WorkspaceId};

#[test]
fn upsert_workspace_persists_and_round_trips_by_id_and_root() {
    let path = test_db_path("workspaces-upsert");
    let mut store = SqliteStore::open(&path).expect("store should open");

    let workspace = default_test_workspace();
    let workspace_id = workspace.id().clone();
    let root = workspace.root_realpath().as_str().to_string();

    let stored = store
        .upsert_workspace(workspace.clone())
        .expect("workspace upsert");
    assert_eq!(stored.id(), &workspace_id);

    let by_id = store
        .workspace(&workspace_id)
        .expect("workspace read")
        .expect("workspace exists");
    assert_eq!(by_id.id(), &workspace_id);

    let by_root = store
        .workspace_by_root_realpath(&root)
        .expect("workspace read by root")
        .expect("workspace exists by root");
    assert_eq!(by_root.id(), &workspace_id);

    let listed = store.workspaces().expect("list workspaces");
    assert_eq!(listed.len(), 1);

    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_session_open_rejects_unknown_workspace() {
    let path = test_db_path("commit-session-open-rejects-unknown-workspace");
    let mut store = SqliteStore::open(&path).expect("store should open");

    let session_id = SessionId::new("session-without-workspace").expect("session id");
    let unknown_workspace = WorkspaceId::new("workspace-not-yet-persisted").expect("workspace id");

    let error = store
        .commit_session_open(CommitSessionOpen {
            session: SessionProjection {
                id: session_id,
                owner_client_name: "sqlite-tests".to_string(),
                owner_principal_id: "principal-test-owner".to_string(),
                current_session_authority_hash: "session-authority-hash".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Should fail".to_string(),
                status: SessionStatus::Idle,
                workspace_id: unknown_workspace,
            },
            occurred_at_ms: 1,
        })
        .expect_err("commit must reject unknown workspace");

    assert_eq!(
        error,
        StoreError::SessionWorkspaceMissing {
            workspace_id: "workspace-not-yet-persisted".to_string(),
        }
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn upsert_workspace_replaces_metadata_for_same_id() {
    let path = test_db_path("workspaces-upsert-replace");
    let mut store = SqliteStore::open(&path).expect("store should open");

    let original = default_test_workspace();
    let id = original.id().clone();
    store.upsert_workspace(original).expect("initial upsert");

    let mut updated = test_workspace(id.as_str(), "/");
    updated.0.display_name = "Renamed Workspace".to_string();
    store.upsert_workspace(updated).expect("upsert overwrite");

    let stored = store
        .workspace(&id)
        .expect("workspace read")
        .expect("workspace exists");
    assert_eq!(stored.display_name(), "Renamed Workspace");

    let _ = std::fs::remove_file(path);
}

#[test]
fn workspace_id_helper_returns_default_test_id() {
    assert_eq!(
        default_test_workspace_id().as_str(),
        crate::DEFAULT_TEST_WORKSPACE_ID
    );
}
