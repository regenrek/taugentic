use ta_protocol::wire::{TrustState, Workspace, WorkspaceId, WorkspacePath};

use crate::{
    ArtifactRecord, EventRecord, PrincipalProjection, RunProjection, SessionProjection, StoreError,
    WorkspaceProjection,
};

pub trait StoreSeedRepository {
    fn append_event(&mut self, event: EventRecord) -> Result<(), StoreError>;
    fn save_principal(&mut self, principal: PrincipalProjection) -> Result<(), StoreError>;
    fn save_workspace(&mut self, workspace: WorkspaceProjection) -> Result<(), StoreError>;
    fn save_session(&mut self, session: SessionProjection) -> Result<(), StoreError>;
    fn save_run(&mut self, run: RunProjection) -> Result<(), StoreError>;
    fn save_artifact(&mut self, artifact: ArtifactRecord) -> Result<(), StoreError>;
}

/// Stable default workspace identifier seeded by every test. Tests that need
/// distinct workspaces should call [`seed_test_workspace`] with an explicit id.
pub const DEFAULT_TEST_WORKSPACE_ID: &str = "workspace-test-default";

/// Build a deterministic test workspace projection rooted at `root`. Tests
/// must persist this via [`StoreSeedRepository::save_workspace`] before
/// inserting any session that references it.
pub fn test_workspace(id: &str, root: &str) -> WorkspaceProjection {
    let workspace = Workspace {
        id: WorkspaceId::new(id).expect("test workspace id"),
        root_realpath: WorkspacePath::from_canonical_wire_value(root)
            .expect("test workspace root must be absolute and canonical"),
        display_name: "Test Workspace".to_string(),
        trust_state: TrustState::Unverified,
        git_repo_root: None,
        created_at: "1970-01-01T00:00:00Z".to_string(),
        last_used_at: "1970-01-01T00:00:00Z".to_string(),
    };
    WorkspaceProjection::new(workspace)
}

/// Platform-specific canonical root used by the default test workspace.
///
/// Workspaces require an absolute, lexically canonical path; `/` is absolute
/// on Unix but not on Windows, so the helper picks a stable per-OS fallback
/// that the wire-shape validator accepts without filesystem IO.
pub fn default_test_workspace_root() -> &'static str {
    if cfg!(windows) { r"C:\" } else { "/" }
}

/// Convenience wrapper around [`test_workspace`] using the default id and a
/// platform-appropriate canonical root (`/` on Unix, `C:\` on Windows).
pub fn default_test_workspace() -> WorkspaceProjection {
    test_workspace(DEFAULT_TEST_WORKSPACE_ID, default_test_workspace_root())
}

/// Seed `store` with the default test workspace and return its id. Idempotent
/// for tests that may seed multiple sessions.
pub fn seed_default_test_workspace<S: StoreSeedRepository>(
    store: &mut S,
) -> Result<WorkspaceId, StoreError> {
    let workspace = default_test_workspace();
    let id = workspace.id().clone();
    let _ = store.save_workspace(workspace);
    Ok(id)
}

/// Returns the default test workspace id without touching the store. Tests
/// that intentionally seed via the production commit path should still call
/// [`seed_default_test_workspace`] to create the row first.
pub fn default_test_workspace_id() -> WorkspaceId {
    WorkspaceId::new(DEFAULT_TEST_WORKSPACE_ID).expect("default test workspace id")
}
