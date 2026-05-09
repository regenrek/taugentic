use std::path::{Component, Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::wire::{RunId, identifier};

identifier!(WorkspaceId, "workspace");

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "code")]
#[ts(export_to = "generated/")]
pub enum WorkspacePathError {
    #[error("WorkspacePathNotAbsolute: {path}")]
    WorkspacePathNotAbsolute { path: String },
    #[error("WorkspacePathNotCanonical: {path}")]
    WorkspacePathNotCanonical {
        path: String,
        canonical: Option<String>,
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, JsonSchema, TS)]
#[schemars(transparent)]
#[ts(export_to = "generated/")]
pub struct WorkspacePath(String);

impl WorkspacePath {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, WorkspacePathError> {
        let path = path.into();
        ensure_absolute(&path)?;
        let canonical = ta_host_platform::canonical_realpath(&path).map_err(|error| {
            WorkspacePathError::WorkspacePathNotCanonical {
                path: display_path(&path),
                canonical: None,
                reason: Some(error.to_string()),
            }
        })?;
        if canonical != path {
            return Err(WorkspacePathError::WorkspacePathNotCanonical {
                path: display_path(&path),
                canonical: Some(display_path(&canonical)),
                reason: None,
            });
        }

        Ok(Self(display_path(path)))
    }

    pub fn canonicalize_existing(path: impl Into<PathBuf>) -> Result<Self, WorkspacePathError> {
        let path = path.into();
        ensure_absolute(&path)?;
        let canonical = ta_host_platform::canonical_realpath(&path).map_err(|error| {
            WorkspacePathError::WorkspacePathNotCanonical {
                path: display_path(&path),
                canonical: None,
                reason: Some(error.to_string()),
            }
        })?;

        Ok(Self(display_path(canonical)))
    }

    pub fn from_canonical_wire_value(value: impl Into<String>) -> Result<Self, WorkspacePathError> {
        let value = value.into();
        let path = Path::new(&value);
        ensure_absolute(path)?;
        if path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(WorkspacePathError::WorkspacePathNotCanonical {
                path: value,
                canonical: None,
                reason: Some("path contains relative components".to_string()),
            });
        }

        Ok(Self(value))
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<WorkspacePath> for String {
    fn from(value: WorkspacePath) -> Self {
        value.0
    }
}

impl<'de> Deserialize<'de> for WorkspacePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_canonical_wire_value(value).map_err(serde::de::Error::custom)
    }
}

fn ensure_absolute(path: &Path) -> Result<(), WorkspacePathError> {
    if path.is_absolute() {
        return Ok(());
    }

    Err(WorkspacePathError::WorkspacePathNotAbsolute {
        path: display_path(path),
    })
}

fn display_path(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().into_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct Workspace {
    pub id: WorkspaceId,
    pub root_realpath: WorkspacePath,
    pub display_name: String,
    pub trust_state: TrustState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_repo_root: Option<WorkspacePath>,
    pub created_at: String,
    pub last_used_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum TrustState {
    Unverified,
    UserConfirmed {
        #[serde(rename = "confirmedAt")]
        #[ts(rename = "confirmedAt")]
        confirmed_at: String,
    },
    Revoked,
}

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
