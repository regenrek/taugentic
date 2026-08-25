use ta_protocol::wire::{
    ApprovalScope, ExecutionContext, NetworkPolicy, PermissionPolicy, ProcessExecPolicy,
};

use crate::{Operation, PolicyDecision};

/// Evaluates an operation against the immutable policy snapshot persisted on a run.
///
/// Runtime-profile policy is only an input while the snapshot is compiled. Once a run exists,
/// every execution path must use this function instead of consulting mutable profile state.
pub fn evaluate_execution_context(
    context: &ExecutionContext,
    operation: &Operation,
) -> PolicyDecision {
    if let Some(reason) = denied_reason(context, operation) {
        return PolicyDecision::Deny { reason };
    }

    match context.permission_policy {
        PermissionPolicy::WorkspaceWriteWithApproval | PermissionPolicy::RepoWriteWithApproval => {
            PolicyDecision::RequireApproval {
                reason: approval_reason(operation),
            }
        }
        PermissionPolicy::ReadOnly
        | PermissionPolicy::WorkspaceWrite
        | PermissionPolicy::Unrestricted => PolicyDecision::Allow,
    }
}

fn denied_reason(context: &ExecutionContext, operation: &Operation) -> Option<String> {
    let denied = match operation.scope {
        ApprovalScope::FileWrite => {
            matches!(context.permission_policy, PermissionPolicy::ReadOnly)
                || context.sandbox_profile.write_roots.is_empty()
        }
        ApprovalScope::ProcessExec => matches!(
            context.sandbox_profile.process_exec,
            ProcessExecPolicy::Denied
        ),
        ApprovalScope::NetworkAccess => {
            matches!(context.network_policy, NetworkPolicy::None)
        }
    };
    denied.then(|| {
        format!(
            "{} is denied by the frozen execution context",
            operation.label
        )
    })
}

fn approval_reason(operation: &Operation) -> String {
    match operation.scope {
        ApprovalScope::FileWrite => format!("{} writes to the workspace", operation.label),
        ApprovalScope::ProcessExec => format!("{} executes a process", operation.label),
        ApprovalScope::NetworkAccess => {
            format!("{} requires network access", operation.label)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_protocol::wire::{
        EnvPolicy, SandboxProfile, WorkspaceId, WorkspacePath, WorkspaceScope,
    };

    #[test]
    fn frozen_context_is_the_only_operation_policy_input() {
        let mut context = test_context();
        let operation = Operation::new(ApprovalScope::FileWrite, "apply patch");

        assert_eq!(
            evaluate_execution_context(&context, &operation),
            PolicyDecision::RequireApproval {
                reason: "apply patch writes to the workspace".to_string()
            }
        );

        context.permission_policy = PermissionPolicy::ReadOnly;
        assert_eq!(
            evaluate_execution_context(&context, &operation),
            PolicyDecision::Deny {
                reason: "apply patch is denied by the frozen execution context".to_string()
            }
        );
    }

    #[test]
    fn scope_specific_denials_do_not_disable_unrelated_operations() {
        let mut context = test_context();
        context.permission_policy = PermissionPolicy::ReadOnly;
        context.sandbox_profile.write_roots.clear();

        assert_eq!(
            evaluate_execution_context(
                &context,
                &Operation::new(ApprovalScope::ProcessExec, "inspect repository")
            ),
            PolicyDecision::Allow
        );
    }

    fn test_context() -> ExecutionContext {
        let root = WorkspacePath::canonicalize_existing(
            std::env::current_dir().expect("current directory"),
        )
        .expect("workspace path");
        ExecutionContext {
            workspace_id: WorkspaceId::new("workspace-policy").expect("workspace id"),
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
            permission_policy: PermissionPolicy::WorkspaceWriteWithApproval,
            network_policy: NetworkPolicy::Open,
            env_policy: EnvPolicy::workspace_default(),
        }
    }
}
