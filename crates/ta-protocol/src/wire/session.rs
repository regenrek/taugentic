use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{DaemonEventCursor, SessionAuthority, SessionId, WorkspaceId, WorkspacePath};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum SessionStatus {
    Idle,
    Running,
    Paused,
    Failed,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: String,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonSessionAttachParams {
    pub session_id: SessionId,
    pub session_authority: SessionAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonSessionOpenParams {
    pub title: String,
    pub workspace: WorkspaceSelector,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export_to = "generated/")]
pub enum WorkspaceSelector {
    ByPath {
        path: WorkspacePath,
        trust_acknowledged: bool,
    },
    ById {
        id: WorkspaceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonSessionOpenResult {
    pub session: SessionSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_cursor: Option<DaemonEventCursor>,
    pub session_authority: SessionAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct DaemonSessionAttachResult {
    pub session: SessionSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_cursor: Option<DaemonEventCursor>,
    pub session_authority: SessionAuthority,
}
