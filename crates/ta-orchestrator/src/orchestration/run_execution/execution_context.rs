use ta_policy::{Operation, PolicyDecision, PolicyEngine};
use ta_protocol::wire::{
    ApprovalScope, ConflictSummary, EnvPolicy, ExecutionContext, NetworkPolicy, PermissionPolicy,
    ProcessExecPolicy, RuntimePolicyMode, RuntimeProfileSummary, SandboxProfile, TrustState,
    WorkspaceCapabilityUnsupported, WorkspaceId, WorkspaceMode, WorkspacePath, WorkspaceScope,
    WorktreeCleanupPolicy, WorktreeInfo,
};
use ta_store::{PersistenceStore, WorkspaceProjection};

use super::*;

#[derive(Debug, Clone)]
pub(super) struct ExecutionContextRequest {
    pub workspace_mode: WorkspaceMode,
    pub cleanup_policy: WorktreeCleanupPolicy,
    pub planned_write_files: Vec<String>,
}

impl ExecutionContextRequest {
    pub fn workspace_write() -> Self {
        Self {
            workspace_mode: WorkspaceMode::WorkspaceWrite,
            cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
            planned_write_files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PreparedExecutionContext {
    pub execution_context: ExecutionContext,
    pub workspace_info: Option<WorktreeInfo>,
    pub claimed_files: Vec<String>,
    pub conflict_warning: Option<ta_protocol::wire::ConflictWarning>,
    pub conflict_summary: Option<ConflictSummary>,
}

struct ExecutionWorkspaceInput<'a> {
    workspace_id: &'a WorkspaceId,
    workspace_root: &'a WorkspacePath,
    parent_repo: &'a WorkspacePath,
    artifact_root: WorkspacePath,
    request: ExecutionContextRequest,
    compiled_policy: CompiledExecutionPolicy,
    env_policy: EnvPolicy,
    denied_roots: Vec<WorkspacePath>,
}

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(super) fn prepare_execution_context(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        runtime_profile: &RuntimeProfileSummary,
        request: ExecutionContextRequest,
    ) -> Result<PreparedExecutionContext, RunExecutionError> {
        reject_unsupported_scope(request.workspace_mode)?;
        let workspace = self.session_workspace(session_id)?;

        if !matches!(workspace.trust_state(), TrustState::UserConfirmed { .. }) {
            return Err(RunExecutionError::WorkspaceTrustRequired(
                workspace.id().as_str().to_string(),
            ));
        }

        let workspace_root = workspace.root_realpath().clone();
        let parent_repo = workspace
            .git_repo_root()
            .unwrap_or_else(|| workspace.root_realpath());
        let artifact_root = prepare_artifact_root(self.runtime.artifact_root())?;
        let compiled_policy = compile_execution_policy(
            request.workspace_mode,
            runtime_profile.policy_mode,
            self.runtime.supports_network(),
        );
        self.prepare_execution_context_from_workspace(
            run_id,
            ExecutionWorkspaceInput {
                workspace_id: workspace.id(),
                workspace_root: &workspace_root,
                parent_repo,
                artifact_root,
                request,
                compiled_policy,
                env_policy: EnvPolicy::workspace_default(),
                denied_roots: Vec::new(),
            },
        )
    }

    pub(super) fn prepare_child_execution_context(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        parent: &ExecutionContext,
        request: ExecutionContextRequest,
    ) -> Result<PreparedExecutionContext, RunExecutionError> {
        reject_unsupported_scope(request.workspace_mode)?;
        let workspace = self.session_workspace(session_id)?;
        if workspace.id() != &parent.workspace_id
            || workspace.root_realpath() != &parent.workspace_root
        {
            return Err(context_inheritance_unsupported(
                format!("{:?}", request.workspace_mode),
                "the parent execution context no longer matches the session workspace",
            ));
        }
        let parent_repo = workspace
            .git_repo_root()
            .unwrap_or_else(|| workspace.root_realpath());
        let compiled_policy =
            compile_child_execution_policy(parent, request.workspace_mode, parent_repo)?;

        self.prepare_execution_context_from_workspace(
            run_id,
            ExecutionWorkspaceInput {
                workspace_id: &parent.workspace_id,
                workspace_root: &parent.workspace_root,
                parent_repo,
                artifact_root: parent.artifact_root.clone(),
                request,
                compiled_policy,
                env_policy: parent.env_policy.clone(),
                denied_roots: parent.sandbox_profile.denied_roots.clone(),
            },
        )
    }

    fn session_workspace(
        &self,
        session_id: &crate::SessionId,
    ) -> Result<WorkspaceProjection, RunExecutionError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        let session = store
            .session(session_id)?
            .ok_or_else(|| RunExecutionError::SessionNotFound(session_id.as_str().to_string()))?;
        store.workspace(&session.workspace_id)?.ok_or_else(|| {
            RunExecutionError::SessionWorkspaceNotFound(session.workspace_id.as_str().to_string())
        })
    }

    fn prepare_execution_context_from_workspace(
        &self,
        run_id: &RunId,
        input: ExecutionWorkspaceInput<'_>,
    ) -> Result<PreparedExecutionContext, RunExecutionError> {
        let ExecutionWorkspaceInput {
            workspace_id,
            workspace_root,
            parent_repo,
            artifact_root,
            request,
            compiled_policy,
            env_policy,
            denied_roots,
        } = input;
        let dispatch = self
            .runtime
            .allocate_execution_workspace(
                run_id,
                parent_repo.as_path(),
                workspace_root.as_path(),
                request.workspace_mode,
                request.cleanup_policy,
                &request.planned_write_files,
            )
            .map_err(map_agent_runtime_error)?;
        let effective_cwd = workspace_path(&dispatch.effective_cwd)?;
        let workspace_scope = workspace_scope(
            request.workspace_mode,
            workspace_root,
            parent_repo,
            &effective_cwd,
            dispatch.worktree_info.as_ref(),
        )?;
        let sandbox_profile = sandbox_profile(
            workspace_root,
            &effective_cwd,
            &artifact_root,
            parent_repo,
            &compiled_policy.permission_policy,
            compiled_policy.process_exec,
            denied_roots,
        );
        let conflict_summary = dispatch
            .conflict_warning
            .as_ref()
            .map(conflict_summary_for_warning);

        Ok(PreparedExecutionContext {
            execution_context: ExecutionContext {
                workspace_id: workspace_id.clone(),
                workspace_root: workspace_root.clone(),
                effective_cwd,
                artifact_root,
                workspace_scope,
                sandbox_profile,
                permission_policy: compiled_policy.permission_policy,
                network_policy: compiled_policy.network_policy,
                env_policy,
            },
            workspace_info: dispatch.worktree_info,
            claimed_files: dispatch.claimed_files,
            conflict_warning: dispatch.conflict_warning,
            conflict_summary,
        })
    }
}

fn reject_unsupported_scope(workspace_mode: WorkspaceMode) -> Result<(), RunExecutionError> {
    match workspace_mode {
        WorkspaceMode::RemoteWorker => Err(RunExecutionError::WorkspaceScopeUnsupported(
            "remoteWorker".to_string(),
        )),
        WorkspaceMode::Containerized => Err(RunExecutionError::WorkspaceScopeUnsupported(
            "containerized".to_string(),
        )),
        WorkspaceMode::Ephemeral => Err(RunExecutionError::WorkspaceScopeUnsupported(
            "ephemeral".to_string(),
        )),
        WorkspaceMode::Readonly
        | WorkspaceMode::WorkspaceWrite
        | WorkspaceMode::WorktreeWrite
        | WorkspaceMode::RepoWriteWithApproval => Ok(()),
    }
}

fn workspace_scope(
    workspace_mode: WorkspaceMode,
    workspace_root: &WorkspacePath,
    repo_root: &WorkspacePath,
    effective_cwd: &WorkspacePath,
    worktree_info: Option<&WorktreeInfo>,
) -> Result<WorkspaceScope, RunExecutionError> {
    match workspace_mode {
        WorkspaceMode::Readonly => Ok(WorkspaceScope::Readonly {
            root: workspace_root.clone(),
        }),
        WorkspaceMode::WorkspaceWrite => Ok(WorkspaceScope::Local {
            root: workspace_root.clone(),
        }),
        WorkspaceMode::RepoWriteWithApproval => Ok(WorkspaceScope::Local {
            root: repo_root.clone(),
        }),
        WorkspaceMode::WorktreeWrite => {
            let worktree_info = worktree_info.ok_or_else(|| {
                RunExecutionError::ExecutionContextPathInvalid(
                    "worktree scope has no prepared worktree".to_string(),
                )
            })?;
            Ok(WorkspaceScope::Worktree {
                root: workspace_root.clone(),
                worktree: effective_cwd.clone(),
                branch: worktree_info.branch.clone(),
            })
        }
        WorkspaceMode::RemoteWorker | WorkspaceMode::Containerized | WorkspaceMode::Ephemeral => {
            Err(RunExecutionError::WorkspaceScopeUnsupported(format!(
                "{workspace_mode:?}"
            )))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompiledExecutionPolicy {
    permission_policy: PermissionPolicy,
    process_exec: ProcessExecPolicy,
    network_policy: NetworkPolicy,
}

fn compile_execution_policy(
    workspace_mode: WorkspaceMode,
    runtime_policy_mode: RuntimePolicyMode,
    supports_network: bool,
) -> CompiledExecutionPolicy {
    let engine = PolicyEngine::from_runtime_policy_mode(runtime_policy_mode);
    let file_write = engine.evaluate(
        &Operation::new(ApprovalScope::FileWrite, "write workspace files"),
        supports_network,
    );
    let process_exec = engine.evaluate(
        &Operation::new(ApprovalScope::ProcessExec, "execute workspace processes"),
        supports_network,
    );
    let network_access = engine.evaluate(
        &Operation::new(ApprovalScope::NetworkAccess, "access the network"),
        supports_network,
    );

    let permission_policy = match workspace_mode {
        WorkspaceMode::Readonly => PermissionPolicy::ReadOnly,
        WorkspaceMode::RepoWriteWithApproval => match file_write {
            PolicyDecision::Deny { .. } => PermissionPolicy::ReadOnly,
            PolicyDecision::RequireApproval { .. } | PolicyDecision::Allow => {
                PermissionPolicy::RepoWriteWithApproval
            }
        },
        WorkspaceMode::WorkspaceWrite | WorkspaceMode::WorktreeWrite => match file_write {
            PolicyDecision::Deny { .. } => PermissionPolicy::ReadOnly,
            PolicyDecision::RequireApproval { .. } => PermissionPolicy::WorkspaceWriteWithApproval,
            PolicyDecision::Allow => PermissionPolicy::WorkspaceWrite,
        },
        WorkspaceMode::RemoteWorker | WorkspaceMode::Containerized | WorkspaceMode::Ephemeral => {
            PermissionPolicy::ReadOnly
        }
    };
    let process_exec = match process_exec {
        PolicyDecision::Deny { .. } => ProcessExecPolicy::Denied,
        PolicyDecision::RequireApproval { .. } | PolicyDecision::Allow => {
            ProcessExecPolicy::AllowAll
        }
    };
    let network_policy = match network_access {
        PolicyDecision::Deny { .. } => NetworkPolicy::None,
        PolicyDecision::RequireApproval { .. } | PolicyDecision::Allow => NetworkPolicy::Open,
    };

    CompiledExecutionPolicy {
        permission_policy,
        process_exec,
        network_policy,
    }
}

fn sandbox_profile(
    workspace_root: &WorkspacePath,
    effective_cwd: &WorkspacePath,
    artifact_root: &WorkspacePath,
    repo_root: &WorkspacePath,
    permission_policy: &PermissionPolicy,
    process_exec: ProcessExecPolicy,
    denied_roots: Vec<WorkspacePath>,
) -> SandboxProfile {
    let mut read_roots = vec![workspace_root.clone()];
    push_unique(&mut read_roots, effective_cwd);
    let mut write_roots = Vec::new();
    match permission_policy {
        PermissionPolicy::ReadOnly => {}
        PermissionPolicy::RepoWriteWithApproval | PermissionPolicy::Unrestricted => {
            push_unique(&mut read_roots, repo_root);
            push_unique(&mut write_roots, repo_root);
            push_unique(&mut write_roots, artifact_root);
        }
        PermissionPolicy::WorkspaceWrite | PermissionPolicy::WorkspaceWriteWithApproval => {
            push_unique(&mut write_roots, effective_cwd);
            push_unique(&mut write_roots, artifact_root);
        }
    }

    SandboxProfile {
        read_roots,
        write_roots,
        denied_roots,
        process_exec,
    }
}

fn compile_child_execution_policy(
    parent: &ExecutionContext,
    workspace_mode: WorkspaceMode,
    repo_root: &WorkspacePath,
) -> Result<CompiledExecutionPolicy, RunExecutionError> {
    if !path_allowed(&parent.sandbox_profile.read_roots, &parent.workspace_root) {
        return Err(context_inheritance_unsupported(
            format!("{workspace_mode:?}"),
            "the parent context does not grant read access to its workspace root",
        ));
    }

    let permission_policy = match workspace_mode {
        WorkspaceMode::Readonly => PermissionPolicy::ReadOnly,
        WorkspaceMode::WorkspaceWrite => {
            require_parent_write(parent, workspace_mode, &parent.workspace_root)?;
            child_workspace_write_permission(parent.permission_policy)
        }
        WorkspaceMode::WorktreeWrite => {
            require_parent_write(parent, workspace_mode, &parent.effective_cwd)?;
            child_workspace_write_permission(parent.permission_policy)
        }
        WorkspaceMode::RepoWriteWithApproval => {
            require_parent_write(parent, workspace_mode, repo_root)?;
            PermissionPolicy::RepoWriteWithApproval
        }
        WorkspaceMode::RemoteWorker | WorkspaceMode::Containerized | WorkspaceMode::Ephemeral => {
            return Err(context_inheritance_unsupported(
                format!("{workspace_mode:?}"),
                "the requested child workspace scope is not implemented",
            ));
        }
    };

    Ok(CompiledExecutionPolicy {
        permission_policy,
        process_exec: parent.sandbox_profile.process_exec.clone(),
        network_policy: parent.network_policy.clone(),
    })
}

fn child_workspace_write_permission(parent: PermissionPolicy) -> PermissionPolicy {
    match parent {
        PermissionPolicy::WorkspaceWriteWithApproval | PermissionPolicy::RepoWriteWithApproval => {
            PermissionPolicy::WorkspaceWriteWithApproval
        }
        PermissionPolicy::WorkspaceWrite | PermissionPolicy::Unrestricted => {
            PermissionPolicy::WorkspaceWrite
        }
        PermissionPolicy::ReadOnly => PermissionPolicy::ReadOnly,
    }
}

fn require_parent_write(
    parent: &ExecutionContext,
    workspace_mode: WorkspaceMode,
    path: &WorkspacePath,
) -> Result<(), RunExecutionError> {
    if matches!(parent.permission_policy, PermissionPolicy::ReadOnly)
        || !path_allowed(&parent.sandbox_profile.write_roots, path)
    {
        return Err(context_inheritance_unsupported(
            format!("{workspace_mode:?}"),
            "the requested child scope would widen the parent filesystem authority",
        ));
    }
    Ok(())
}

fn path_allowed(roots: &[WorkspacePath], path: &WorkspacePath) -> bool {
    roots
        .iter()
        .any(|root| path.as_path().starts_with(root.as_path()))
}

pub(super) fn workspace_mode_for_fork(
    parent: &ExecutionContext,
) -> Result<WorkspaceMode, RunExecutionError> {
    match parent.workspace_scope {
        WorkspaceScope::Readonly { .. } => Ok(WorkspaceMode::Readonly),
        WorkspaceScope::Worktree { .. } => Ok(WorkspaceMode::WorktreeWrite),
        WorkspaceScope::Local { .. }
            if matches!(
                parent.permission_policy,
                PermissionPolicy::RepoWriteWithApproval
            ) =>
        {
            Ok(WorkspaceMode::RepoWriteWithApproval)
        }
        WorkspaceScope::Local { .. } => Ok(WorkspaceMode::WorkspaceWrite),
        WorkspaceScope::Remote { .. } => Err(context_inheritance_unsupported(
            "remote",
            "the parent workspace scope cannot be forked by the native harness",
        )),
        WorkspaceScope::Container { .. } => Err(context_inheritance_unsupported(
            "container",
            "the parent workspace scope cannot be forked by the native harness",
        )),
        WorkspaceScope::Ephemeral { .. } => Err(context_inheritance_unsupported(
            "ephemeral",
            "the parent workspace scope cannot be forked by the native harness",
        )),
    }
}

fn context_inheritance_unsupported(
    requested: impl Into<String>,
    reason: impl Into<String>,
) -> RunExecutionError {
    RunExecutionError::WorkspaceCapabilityUnsupported(WorkspaceCapabilityUnsupported {
        variant: None,
        vendor: None,
        capability: "contextInheritance".to_string(),
        requested: requested.into(),
        reason: reason.into(),
    })
}

fn push_unique(paths: &mut Vec<WorkspacePath>, path: &WorkspacePath) {
    if !paths.contains(path) {
        paths.push(path.clone());
    }
}

fn workspace_path(path: impl AsRef<std::path::Path>) -> Result<WorkspacePath, RunExecutionError> {
    WorkspacePath::canonicalize_existing(path.as_ref().to_path_buf())
        .map_err(|error| RunExecutionError::ExecutionContextPathInvalid(error.to_string()))
}

fn prepare_artifact_root(path: &std::path::Path) -> Result<WorkspacePath, RunExecutionError> {
    std::fs::create_dir_all(path).map_err(|error| {
        RunExecutionError::ExecutionContextPathInvalid(format!(
            "artifact root {} could not be created: {error}",
            path.display()
        ))
    })?;
    workspace_path(path)
}

fn conflict_summary_for_warning(warning: &ta_protocol::wire::ConflictWarning) -> ConflictSummary {
    let mut files = warning
        .conflicts
        .iter()
        .map(|conflict| conflict.file.clone())
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    ConflictSummary {
        warning_count: warning.conflicts.len() as u32,
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_policy_compilation_uses_policy_engine_decisions() {
        assert_eq!(
            compile_execution_policy(WorkspaceMode::WorkspaceWrite, RuntimePolicyMode::Deny, true,),
            CompiledExecutionPolicy {
                permission_policy: PermissionPolicy::ReadOnly,
                process_exec: ProcessExecPolicy::Denied,
                network_policy: NetworkPolicy::None,
            }
        );
        assert_eq!(
            compile_execution_policy(
                WorkspaceMode::WorkspaceWrite,
                RuntimePolicyMode::RequireApproval,
                true,
            ),
            CompiledExecutionPolicy {
                permission_policy: PermissionPolicy::WorkspaceWriteWithApproval,
                process_exec: ProcessExecPolicy::AllowAll,
                network_policy: NetworkPolicy::Open,
            }
        );
        assert_eq!(
            compile_execution_policy(
                WorkspaceMode::RepoWriteWithApproval,
                RuntimePolicyMode::Deny,
                true,
            )
            .permission_policy,
            PermissionPolicy::ReadOnly
        );
        assert_eq!(
            compile_execution_policy(
                WorkspaceMode::WorkspaceWrite,
                RuntimePolicyMode::Allow,
                false,
            )
            .network_policy,
            NetworkPolicy::None
        );
    }
}
