use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{ProjectId, WorkspaceId, WorkspacePath, u64_string};

pub const WORKSPACE_FILE_TREE_MAX_ENTRIES: usize = 20_000;
pub const WORKSPACE_FILE_ATTACHMENT_MAX_COUNT: usize = 20;
pub const WORKSPACE_IMAGE_ATTACHMENT_MAX_COUNT: usize = 10;
pub const WORKSPACE_IMAGE_ATTACHMENT_MAX_BYTES: u64 = 16 * 1024 * 1024;
pub const WORKSPACE_IMAGE_ATTACHMENT_MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
pub const WORKSPACE_TEXT_MAX_BYTES: u64 = 4 * 1024 * 1024;
pub const WORKSPACE_BINARY_MAX_BYTES: u64 = 16 * 1024 * 1024;
pub const WORKSPACE_PDF_MAX_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum WorkspaceFileKind {
    Directory,
    Text,
    Image,
    Pdf,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileEntry {
    pub path: String,
    pub name: String,
    pub kind: WorkspaceFileKind,
    pub is_symlink: bool,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileAttachmentRequest {
    pub path: String,
    pub expected_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileAttachment {
    pub path: String,
    pub revision: String,
    pub kind: WorkspaceFileKind,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub byte_len: u64,
}

/// An ephemeral native-renderer source materialized from a daemon-validated
/// image query. The source is owned and cleaned up by the desktop bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct NativeImagePreview {
    pub source: String,
    pub media_type: String,
    pub revision: String,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileTreeParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileTreeResult {
    pub entries: Vec<WorkspaceFileEntry>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileReadParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf_page_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export_to = "generated/")]
pub enum BoundedFileContent {
    Text {
        text: String,
        revision: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(type = "string")]
        byte_len: u64,
    },
    Image {
        data_uri: String,
        media_type: String,
        revision: String,
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(type = "string")]
        byte_len: u64,
    },
    Pdf {
        preview_data_uri: String,
        page_index: u32,
        page_count: u32,
        revision: String,
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(type = "string")]
        byte_len: u64,
    },
    Binary {
        data_base64: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        media_type: Option<String>,
        revision: String,
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(type = "string")]
        byte_len: u64,
    },
}

impl BoundedFileContent {
    pub fn revision(&self) -> &str {
        match self {
            Self::Text { revision, .. }
            | Self::Image { revision, .. }
            | Self::Pdf { revision, .. }
            | Self::Binary { revision, .. } => revision,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileReadResult {
    pub path: String,
    pub content: BoundedFileContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileWriteParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub path: String,
    pub expected_revision: String,
    pub text: String,
    pub user_approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileWriteResult {
    pub path: String,
    pub revision: String,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileOpenExternalParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkspaceFileOpenExternalResult {
    pub path: WorkspacePath,
}
