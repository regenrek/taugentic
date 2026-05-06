use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::RunId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum WorkspaceMode {
    Readonly,
    WorkspaceWrite,
    WorktreeWrite,
    RepoWriteWithApproval,
    RemoteWorker,
    Containerized,
    Ephemeral,
}

impl Default for WorkspaceMode {
    fn default() -> Self {
        Self::WorktreeWrite
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum WorktreeCleanupPolicy {
    DeleteOnSuccess,
    DeleteOnTerminal,
    Keep,
    Manual,
}

impl Default for WorktreeCleanupPolicy {
    fn default() -> Self {
        Self::DeleteOnSuccess
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: String,
    pub cleanup_policy: WorktreeCleanupPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum FileClaimKind {
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ConflictSeverity {
    Informational,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct FileClaimConflict {
    pub file: String,
    pub holding_capsule: RunId,
    pub holding_kind: FileClaimKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ConflictWarning {
    pub requesting_capsule: RunId,
    pub severity: ConflictSeverity,
    pub conflicts: Vec<FileClaimConflict>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ConflictSummary {
    pub warning_count: u32,
    pub files: Vec<String>,
}
