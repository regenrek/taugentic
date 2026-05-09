use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{WorkspaceId, WorkspacePath};

const NATIVE_DEFAULT_ENV_ALLOWLIST: &[&str] = &["PATH", "HOME", "USER", "LANG", "TZ"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ExecutionContext {
    pub workspace_id: WorkspaceId,
    pub workspace_root: WorkspacePath,
    pub effective_cwd: WorkspacePath,
    pub artifact_root: WorkspacePath,
    pub workspace_scope: WorkspaceScope,
    pub sandbox_profile: SandboxProfile,
    pub permission_policy: PermissionPolicy,
    pub network_policy: NetworkPolicy,
    pub env_policy: EnvPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum WorkspaceScope {
    Local {
        root: WorkspacePath,
    },
    Worktree {
        root: WorkspacePath,
        worktree: WorkspacePath,
        branch: String,
    },
    Readonly {
        root: WorkspacePath,
    },
    Remote {
        root: WorkspacePath,
    },
    Container {
        root: WorkspacePath,
    },
    Ephemeral {
        root: WorkspacePath,
    },
}

impl WorkspaceScope {
    pub fn unsupported_dispatch_variant(&self) -> Option<&'static str> {
        match self {
            Self::Remote { .. } => Some("remote"),
            Self::Container { .. } => Some("container"),
            Self::Ephemeral { .. } => Some("ephemeral"),
            Self::Local { .. } | Self::Worktree { .. } | Self::Readonly { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SandboxProfile {
    pub read_roots: Vec<WorkspacePath>,
    pub write_roots: Vec<WorkspacePath>,
    pub denied_roots: Vec<WorkspacePath>,
    pub process_exec: ProcessExecPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ProcessExecPolicy {
    Denied,
    Allowlist { binaries: Vec<String> },
    AllowAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum PermissionPolicy {
    ReadOnly,
    WorkspaceWrite,
    WorkspaceWriteWithApproval,
    RepoWriteWithApproval,
    Unrestricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum NetworkPolicy {
    None,
    Loopback,
    Allowlist { domains: Vec<String> },
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum EnvPolicy {
    Allowlist { vars: Vec<String> },
    All,
}

impl EnvPolicy {
    pub fn native_default() -> Self {
        Self::Allowlist {
            vars: NATIVE_DEFAULT_ENV_ALLOWLIST
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
        }
    }

    pub fn acp_default() -> Self {
        Self::All
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkspaceCapabilityUnsupported {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    pub capability: String,
    pub requested: String,
    pub reason: String,
}
