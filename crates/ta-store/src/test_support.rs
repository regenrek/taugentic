use ta_protocol::wire::{
    AgentRuntimeModelId, AgentRuntimeStrategyId, AuthMethodId, AuthProfileConnectionState,
    AuthProfileId, AuthProfileManagementMode, AuthProfileRef, AuthProfileState, EnvPolicy,
    ExecutionContext, NetworkPolicy, PermissionPolicy, ProcessExecPolicy, RunExecutionRoute,
    RunHarnessKind, RunSource, RuntimeProfileId, SandboxProfile, TrustState, Workspace,
    WorkspaceId, WorkspacePath, WorkspaceScope,
};

use crate::{
    ArtifactRecord, AuthProfileProjection, EventRecord, PrincipalProjection, RunProjection,
    SessionProjection, StoreError, WorkspaceProjection,
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

/// Build a deterministic workspace that passed the same trust gate required by
/// production run creation.
pub fn confirmed_test_workspace(id: &str, root: &str) -> WorkspaceProjection {
    let mut workspace = test_workspace(id, root).into_inner();
    workspace.trust_state = TrustState::UserConfirmed {
        confirmed_at: "1970-01-01T00:00:00Z".to_string(),
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
    confirmed_test_workspace(DEFAULT_TEST_WORKSPACE_ID, default_test_workspace_root())
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

/// Deterministic execution context for tests that seed run projections directly.
pub fn default_test_execution_context() -> ExecutionContext {
    let workspace = default_test_workspace();
    let root = workspace.root_realpath().clone();
    ExecutionContext {
        workspace_id: workspace.id().clone(),
        workspace_root: root.clone(),
        effective_cwd: root.clone(),
        artifact_root: root.clone(),
        workspace_scope: WorkspaceScope::Local { root: root.clone() },
        sandbox_profile: SandboxProfile {
            read_roots: vec![root.clone()],
            write_roots: vec![root.clone()],
            denied_roots: Vec::new(),
            process_exec: ProcessExecPolicy::AllowAll,
        },
        permission_policy: PermissionPolicy::Unrestricted,
        network_policy: NetworkPolicy::Open,
        env_policy: EnvPolicy::workspace_default(),
    }
}

/// Explicit immutable route for tests that seed run projections directly.
pub fn default_test_run_source() -> RunSource {
    RunSource::User {
        route: RunExecutionRoute {
            runtime_profile_id: RuntimeProfileId::new("runtime-test").expect("runtime profile id"),
            provider_id: AgentRuntimeStrategyId::new("provider-test").expect("provider id"),
            harness: RunHarnessKind::Native,
            model_id: Some(AgentRuntimeModelId::new("model-test").expect("model id")),
            auth_profile_id: Some(AuthProfileId::new("profile-test").expect("auth profile id")),
        },
        output_contract: None,
        model_id: None,
        recipe_id: None,
    }
}

/// Connected, non-secret auth metadata for tests exercising explicit route
/// validation. This fixture never supplies credential material or bypasses a
/// provider's production execution path.
pub fn connected_test_auth_profile(
    id: &str,
    auth_method_id: &str,
    provider_id: &str,
) -> AuthProfileProjection {
    AuthProfileProjection {
        profile: AuthProfileState {
            profile: AuthProfileRef {
                id: AuthProfileId::new(id).expect("test auth profile id"),
                auth_method_id: AuthMethodId::new(auth_method_id).expect("test auth method id"),
                provider_id: AgentRuntimeStrategyId::new(provider_id).expect("test provider id"),
                display_name: "Test Auth Profile".to_string(),
                account_hint: None,
                plan_tier: None,
            },
            connection_state: AuthProfileConnectionState::Connected,
            last_error: None,
            management_mode: AuthProfileManagementMode::Interactive,
            can_login: false,
            can_logout: true,
            platform_org_linked: None,
            setup_steps: Vec::new(),
            action: None,
            methods: Vec::new(),
        },
        external_account_id: None,
        order: 0,
        is_default: true,
    }
}
