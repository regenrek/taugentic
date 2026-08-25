use std::path::{Path, PathBuf};

use ta_exec::{NetworkPolicy as ExecNetworkPolicy, SandboxProfile as ExecSandboxProfile};
use ta_protocol::wire::{
    ApprovalScope, EnvPolicy, ExecutionContext, NetworkPolicy, ProcessExecPolicy,
    WorkspaceCapabilityUnsupported,
};

use crate::ExecutionError;

#[derive(Debug, Clone)]
pub(crate) struct NativeExecutionSpec {
    pub cwd: PathBuf,
    pub sandbox_profile: ExecSandboxProfile,
}

pub(crate) struct NativeExecutionRequirements<'a> {
    pub cwd: &'a Path,
    pub adapter_read_roots: &'a [PathBuf],
    pub adapter_write_roots: &'a [PathBuf],
}

impl<'a> NativeExecutionRequirements<'a> {
    pub(crate) fn for_cwd(cwd: &'a Path) -> Self {
        Self {
            cwd,
            adapter_read_roots: &[],
            adapter_write_roots: &[],
        }
    }
}

pub(crate) fn validate_native_execution_context(
    context: &ExecutionContext,
) -> Result<(), ExecutionError> {
    if let Some(variant) = context.workspace_scope.unsupported_dispatch_variant() {
        return Err(capability_unsupported(
            Some(variant),
            "executionScope",
            variant,
            "the native harness only executes local, worktree, and readonly workspace scopes",
        ));
    }
    if !context.sandbox_profile.denied_roots.is_empty() {
        return Err(capability_unsupported(
            None,
            "deniedRoots",
            "nested filesystem deny roots",
            "the native sandbox backends cannot enforce deny roots nested inside granted roots",
        ));
    }
    match &context.sandbox_profile.process_exec {
        ProcessExecPolicy::AllowAll | ProcessExecPolicy::Denied => Ok(()),
        ProcessExecPolicy::Allowlist { .. } => Err(capability_unsupported(
            None,
            "processExec",
            "binary allowlist",
            "the native shell and subprocess sandbox cannot enforce descendant binary allowlists",
        )),
    }
}

pub(crate) fn compile_native_execution_spec(
    context: &ExecutionContext,
    requirements: NativeExecutionRequirements<'_>,
) -> Result<NativeExecutionSpec, ExecutionError> {
    validate_native_execution_context(context)?;
    if matches!(
        context.sandbox_profile.process_exec,
        ProcessExecPolicy::Denied
    ) {
        return Err(ExecutionError::PolicyDenied {
            scope: ApprovalScope::ProcessExec,
            reason: "the frozen execution context denies process execution".to_string(),
        });
    }
    let cwd = canonical_existing("native execution cwd", requirements.cwd)?;
    ensure_cwd_allowed(context, &cwd)?;

    let network = match &context.network_policy {
        NetworkPolicy::None => ExecNetworkPolicy::Off,
        NetworkPolicy::Loopback => ExecNetworkPolicy::Loopback,
        NetworkPolicy::Open => ExecNetworkPolicy::Open,
        NetworkPolicy::Allowlist { .. } => {
            return Err(capability_unsupported(
                None,
                "network",
                "destination domain allowlist",
                "the native sandbox network allowlist does not implement domain-aware enforcement",
            ));
        }
    };
    let mut sandbox_profile = ExecSandboxProfile::new()
        .network(network)
        .child_inherits_tty(false);
    for root in &context.sandbox_profile.read_roots {
        sandbox_profile = sandbox_profile.read_path(root.as_path());
    }
    for root in &context.sandbox_profile.write_roots {
        sandbox_profile = sandbox_profile.write_path(root.as_path());
    }
    for root in requirements.adapter_read_roots {
        ensure_absolute("adapter read root", root)?;
        sandbox_profile = sandbox_profile.read_path(root);
    }
    for root in requirements.adapter_write_roots {
        ensure_absolute("adapter write root", root)?;
        sandbox_profile = sandbox_profile.write_path(root);
    }
    sandbox_profile = match &context.env_policy {
        EnvPolicy::Allowlist { vars } => vars
            .iter()
            .fold(sandbox_profile, |profile, name| profile.env(name)),
        EnvPolicy::All => sandbox_profile.inherit_all_env(),
    };

    Ok(NativeExecutionSpec {
        cwd,
        sandbox_profile,
    })
}

pub(crate) fn validate_http_mcp_policy(context: &ExecutionContext) -> Result<(), ExecutionError> {
    match &context.network_policy {
        NetworkPolicy::Open => Ok(()),
        NetworkPolicy::None => Err(ExecutionError::PolicyDenied {
            scope: ApprovalScope::NetworkAccess,
            reason: "the frozen execution context denies HTTP MCP network access".to_string(),
        }),
        NetworkPolicy::Loopback => Err(capability_unsupported(
            None,
            "network",
            "loopback-only HTTP MCP",
            "the HTTP MCP transport does not enforce redirect destinations",
        )),
        NetworkPolicy::Allowlist { .. } => Err(capability_unsupported(
            None,
            "network",
            "destination allowlist for HTTP MCP",
            "the HTTP MCP transport does not enforce redirect destinations",
        )),
    }
}

fn ensure_cwd_allowed(context: &ExecutionContext, cwd: &Path) -> Result<(), ExecutionError> {
    if context
        .sandbox_profile
        .read_roots
        .iter()
        .any(|root| cwd.starts_with(root.as_path()))
    {
        return Ok(());
    }
    Err(ExecutionError::InvalidConfig(format!(
        "native execution cwd {} is outside the frozen read roots",
        cwd.display()
    )))
}

fn canonical_existing(label: &str, path: &Path) -> Result<PathBuf, ExecutionError> {
    ensure_absolute(label, path)?;
    path.canonicalize().map_err(|error| {
        ExecutionError::InvalidConfig(format!(
            "{label} must exist and canonicalize: {}: {error}",
            path.display()
        ))
    })
}

fn ensure_absolute(label: &str, path: &Path) -> Result<(), ExecutionError> {
    if path.is_absolute() {
        return Ok(());
    }
    Err(ExecutionError::InvalidConfig(format!(
        "{label} must be absolute: {}",
        path.display()
    )))
}

pub(crate) fn capability_unsupported(
    variant: Option<&str>,
    capability: &str,
    requested: &str,
    reason: &str,
) -> ExecutionError {
    ExecutionError::WorkspaceCapabilityUnsupported(WorkspaceCapabilityUnsupported {
        variant: variant.map(str::to_string),
        vendor: Some("taugentic-native".to_string()),
        capability: capability.to_string(),
        requested: requested.to_string(),
        reason: reason.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_protocol::wire::{
        PermissionPolicy, SandboxProfile, WorkspaceId, WorkspacePath, WorkspaceScope,
    };

    fn context() -> ExecutionContext {
        let root = WorkspacePath::canonicalize_existing(
            std::env::current_dir().expect("current directory"),
        )
        .expect("workspace path");
        ExecutionContext {
            workspace_id: WorkspaceId::new("workspace-native-spec").expect("workspace id"),
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
        }
    }

    #[test]
    fn compiles_native_profile_only_from_frozen_policy_and_adapter_roots() {
        let context = context();
        let spec = compile_native_execution_spec(
            &context,
            NativeExecutionRequirements::for_cwd(context.effective_cwd.as_path()),
        )
        .expect("native spec");

        assert_eq!(spec.cwd, context.effective_cwd.as_path());
        assert_eq!(
            spec.sandbox_profile.network_policy(),
            &ExecNetworkPolicy::Off
        );
        assert!(
            spec.sandbox_profile
                .reads_path(context.workspace_root.as_path())
        );
        assert!(
            spec.sandbox_profile
                .writes_path(context.effective_cwd.as_path())
        );
        assert!(spec.sandbox_profile.allows_env("PATH"));
        assert!(!spec.sandbox_profile.allows_env("OPENAI_API_KEY"));
    }

    #[test]
    fn rejects_binary_allowlist_before_spawn() {
        let mut context = context();
        context.sandbox_profile.process_exec = ProcessExecPolicy::Allowlist {
            binaries: vec!["git".to_string()],
        };

        let error = validate_native_execution_context(&context).expect_err("unsupported");

        assert!(matches!(
            error,
            ExecutionError::WorkspaceCapabilityUnsupported(detail)
                if detail.capability == "processExec"
        ));
    }

    #[test]
    fn process_denial_blocks_spawn_without_blocking_the_native_turn_loop() {
        let mut context = context();
        context.sandbox_profile.process_exec = ProcessExecPolicy::Denied;

        validate_native_execution_context(&context).expect("turn loop remains usable");
        let error = compile_native_execution_spec(
            &context,
            NativeExecutionRequirements::for_cwd(context.effective_cwd.as_path()),
        )
        .expect_err("spawn must be denied");

        assert!(matches!(
            error,
            ExecutionError::PolicyDenied {
                scope: ApprovalScope::ProcessExec,
                ..
            }
        ));
    }
}
