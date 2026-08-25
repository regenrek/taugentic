use std::{path::Path, sync::Arc, time::Duration};

use ta_protocol::wire::{
    EnvPolicy, ExecutionContext, NetworkPolicy, PermissionPolicy, ProcessExecPolicy,
    SandboxProfile, WorkspaceId, WorkspacePath, WorkspaceScope,
};
use taugentic_agent::tools::ToolContext;
use tokio_util::sync::CancellationToken;

#[allow(dead_code)]
pub fn context(path: &Path, timeout: Duration) -> ToolContext {
    context_with_cancellation(path, timeout, CancellationToken::new())
}

pub fn context_with_cancellation(
    path: &Path,
    timeout: Duration,
    cancellation_token: CancellationToken,
) -> ToolContext {
    let root = WorkspacePath::canonicalize_existing(path).expect("test workspace path");
    ToolContext {
        execution_context: Arc::new(ExecutionContext {
            workspace_id: WorkspaceId::new("workspace-tool-test").expect("workspace id"),
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
            network_policy: NetworkPolicy::None,
            env_policy: EnvPolicy::workspace_default(),
        }),
        cancellation_token,
        timeout,
        parent_turn_id: None,
    }
}
