use ta_protocol::wire::{TrustState, WorkspacePath};

use super::*;

#[test]
fn open_workspace_requires_trust_acknowledgement_before_persisting() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let dir = tempfile::tempdir().expect("tempdir should create");
    let path = WorkspacePath::canonicalize_existing(dir.path()).expect("workspace path");

    let error = service
        .open_workspace(&OpenWorkspaceRequest {
            path: path.clone(),
            trust_acknowledged: false,
        })
        .expect_err("first open should require trust");

    assert!(matches!(error, AppServiceError::WorkspaceTrustRequired(_)));
    assert!(
        service
            .list_workspaces()
            .expect("workspaces should list")
            .into_iter()
            .all(|workspace| workspace.root_realpath != path)
    );
}

#[test]
fn open_workspace_persists_canonical_trusted_workspace_and_reuses_it() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let dir = tempfile::tempdir().expect("tempdir should create");
    let path = WorkspacePath::canonicalize_existing(dir.path()).expect("workspace path");

    let workspace = service
        .open_workspace(&OpenWorkspaceRequest {
            path: path.clone(),
            trust_acknowledged: true,
        })
        .expect("trusted workspace should open");
    let reopened = service
        .open_workspace(&OpenWorkspaceRequest {
            path,
            trust_acknowledged: false,
        })
        .expect("trusted workspace should reopen without a new acknowledgement");

    assert_eq!(reopened.id, workspace.id);
    assert!(matches!(
        reopened.trust_state,
        TrustState::UserConfirmed { .. }
    ));
}

#[test]
fn open_workspace_rejects_files_before_persistence() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let dir = tempfile::tempdir().expect("tempdir should create");
    let file = dir.path().join("not-a-dir.txt");
    std::fs::write(&file, "not a directory").expect("file should write");
    let path = WorkspacePath::canonicalize_existing(&file).expect("workspace path");

    let error = service
        .open_workspace(&OpenWorkspaceRequest {
            path,
            trust_acknowledged: true,
        })
        .expect_err("file path should be rejected");

    assert!(matches!(error, AppServiceError::WorkspaceNotADirectory(_)));
}

#[test]
fn open_project_creates_one_project_and_reuses_it_on_reopen() {
    let service = AppService::bootstrap().expect("service should bootstrap");
    let dir = tempfile::tempdir().expect("tempdir should create");
    let path = WorkspacePath::canonicalize_existing(dir.path()).expect("workspace path");

    let (project_id, first_snapshot) = service
        .open_project("project-open-owner", path.clone(), true)
        .expect("trusted project should open");
    let (reopened_id, reopened_snapshot) = service
        .open_project("project-open-owner", path, false)
        .expect("trusted project should reopen");

    assert_eq!(reopened_id, project_id);
    assert_eq!(first_snapshot.projects.len(), 1);
    assert_eq!(reopened_snapshot.projects.len(), 1);
    assert_eq!(reopened_snapshot.projects[0].id, project_id);
    assert_eq!(reopened_snapshot.projects[0].workspace_ids.len(), 1);
}
