use std::path::Path;

use ta_protocol::wire::{
    EnvPolicy, ExecutionContext, NetworkPolicy, PermissionPolicy, ProcessExecPolicy,
    SandboxProfile, WorkspaceId, WorkspacePath, WorkspaceScope,
};

pub fn execution_context(work_dir: &Path) -> ExecutionContext {
    let root = WorkspacePath::canonicalize_existing(work_dir).expect("test workspace path");
    ExecutionContext {
        workspace_id: WorkspaceId::new("workspace-acp-test").expect("workspace id"),
        workspace_root: root.clone(),
        effective_cwd: root.clone(),
        artifact_root: root.clone(),
        workspace_scope: WorkspaceScope::Local { root: root.clone() },
        sandbox_profile: SandboxProfile {
            read_roots: vec![root.clone()],
            write_roots: vec![root],
            denied_roots: Vec::new(),
            process_exec: ProcessExecPolicy::AllowAll,
        },
        permission_policy: PermissionPolicy::WorkspaceWrite,
        network_policy: NetworkPolicy::Open,
        env_policy: EnvPolicy::workspace_default(),
    }
}
