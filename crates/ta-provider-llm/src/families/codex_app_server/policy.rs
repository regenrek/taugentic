use serde::Serialize;
use ta_protocol::wire::{ExecutionContext, NetworkPolicy, PermissionPolicy, ProcessExecPolicy};

use super::CodexLlmClientError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CodexTurnPolicy {
    pub approval_policy: &'static str,
    pub sandbox_policy: CodexSandboxPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(super) enum CodexSandboxPolicy {
    ReadOnly {
        network_access: bool,
    },
    WorkspaceWrite {
        writable_roots: Vec<String>,
        network_access: bool,
        exclude_slash_tmp: bool,
        exclude_tmpdir_env_var: bool,
    },
    DangerFullAccess,
}

impl CodexTurnPolicy {
    pub(super) fn from_execution_context(
        context: &ExecutionContext,
    ) -> Result<Self, CodexLlmClientError> {
        if let Some(variant) = context.workspace_scope.unsupported_dispatch_variant() {
            return Err(unsupported("execution scope", variant));
        }
        if !context.sandbox_profile.denied_roots.is_empty() {
            return Err(unsupported("filesystem deny roots", "nested deny roots"));
        }
        match &context.sandbox_profile.process_exec {
            ProcessExecPolicy::AllowAll => {}
            ProcessExecPolicy::Denied => {
                return Err(unsupported("process execution", "denied"));
            }
            ProcessExecPolicy::Allowlist { .. } => {
                return Err(unsupported("process execution", "binary allowlist"));
            }
        }

        let network_access = match &context.network_policy {
            NetworkPolicy::None => false,
            NetworkPolicy::Open => true,
            NetworkPolicy::Loopback => return Err(unsupported("network", "loopback")),
            NetworkPolicy::Allowlist { .. } => {
                return Err(unsupported("network", "destination allowlist"));
            }
        };
        if !network_access
            && matches!(
                context.permission_policy,
                PermissionPolicy::WorkspaceWriteWithApproval
                    | PermissionPolicy::RepoWriteWithApproval
                    | PermissionPolicy::Unrestricted
            )
        {
            return Err(unsupported(
                "network",
                "none with approval escalation or unrestricted filesystem access",
            ));
        }
        let approval_policy = match context.permission_policy {
            PermissionPolicy::WorkspaceWriteWithApproval
            | PermissionPolicy::RepoWriteWithApproval => "on-request",
            PermissionPolicy::ReadOnly
            | PermissionPolicy::WorkspaceWrite
            | PermissionPolicy::Unrestricted => "never",
        };
        let sandbox_policy = match context.permission_policy {
            PermissionPolicy::ReadOnly
            | PermissionPolicy::WorkspaceWriteWithApproval
            | PermissionPolicy::RepoWriteWithApproval => {
                CodexSandboxPolicy::ReadOnly { network_access }
            }
            PermissionPolicy::WorkspaceWrite => CodexSandboxPolicy::WorkspaceWrite {
                writable_roots: context
                    .sandbox_profile
                    .write_roots
                    .iter()
                    .map(|path| path.as_str().to_string())
                    .collect(),
                network_access,
                exclude_slash_tmp: true,
                exclude_tmpdir_env_var: true,
            },
            PermissionPolicy::Unrestricted => CodexSandboxPolicy::DangerFullAccess,
        };

        Ok(Self {
            approval_policy,
            sandbox_policy,
        })
    }
}

fn unsupported(capability: &str, requested: &str) -> CodexLlmClientError {
    CodexLlmClientError::InvalidConfig(format!(
        "Codex app-server cannot enforce requested {capability} policy {requested}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_protocol::wire::{EnvPolicy, SandboxProfile};

    #[test]
    fn workspace_write_policy_maps_without_widening_network() {
        let mut context = test_context();
        context.permission_policy = PermissionPolicy::WorkspaceWrite;
        context.network_policy = NetworkPolicy::None;

        let policy = CodexTurnPolicy::from_execution_context(&context).expect("policy");

        assert_eq!(policy.approval_policy, "never");
        assert!(matches!(
            policy.sandbox_policy,
            CodexSandboxPolicy::WorkspaceWrite {
                network_access: false,
                ..
            }
        ));
    }

    #[test]
    fn approval_policy_starts_read_only_and_fails_when_network_cannot_stay_closed() {
        let mut context = test_context();
        context.permission_policy = PermissionPolicy::WorkspaceWriteWithApproval;

        let policy = CodexTurnPolicy::from_execution_context(&context).expect("policy");
        assert_eq!(policy.approval_policy, "on-request");
        assert!(matches!(
            policy.sandbox_policy,
            CodexSandboxPolicy::ReadOnly {
                network_access: true
            }
        ));

        context.network_policy = NetworkPolicy::None;
        let error = CodexTurnPolicy::from_execution_context(&context)
            .expect_err("approval escalation cannot preserve closed network");
        assert!(matches!(error, CodexLlmClientError::InvalidConfig(_)));
    }

    #[test]
    fn destination_allowlist_fails_before_launch() {
        let mut context = test_context();
        context.network_policy = NetworkPolicy::Allowlist {
            domains: vec!["api.openai.com".to_string()],
        };

        let error = CodexTurnPolicy::from_execution_context(&context).expect_err("unsupported");

        assert!(matches!(error, CodexLlmClientError::InvalidConfig(_)));
    }

    fn test_context() -> ExecutionContext {
        let root = ta_protocol::wire::WorkspacePath::canonicalize_existing(
            std::env::current_dir().expect("current dir"),
        )
        .expect("workspace path");
        ExecutionContext {
            workspace_id: ta_protocol::wire::WorkspaceId::new("workspace-test")
                .expect("workspace id"),
            workspace_root: root.clone(),
            effective_cwd: root.clone(),
            artifact_root: root.clone(),
            workspace_scope: ta_protocol::wire::WorkspaceScope::Local { root: root.clone() },
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
}
