use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentRuntimeSelection, DaemonEventCursor, ProjectId, SessionAuthority, SessionId, WorkspaceId,
    WorkspacePath,
};

/// The daemon-owned route selection for a session's next execution.
///
/// This is deliberately an explicit closed state: a session either has no
/// executable route yet or records the exact selection that the next run must
/// agree with. It is not a desktop draft and it never alters an existing run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export_to = "generated/")]
pub enum SessionNextRunSelection {
    Unselected,
    Selected { selection: AgentRuntimeSelection },
}

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
    pub next_run_selection: SessionNextRunSelection,
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
pub struct DaemonSessionSetNextRunSelectionParams {
    pub selection: SessionNextRunSelection,
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
    ByProject {
        project_id: ProjectId,
        workspace_id: WorkspaceId,
    },
    /// Opens a disposable conversation in an existing workspace. The
    /// placement is chosen by the daemon as part of the same session-open
    /// commit; clients cannot create it as standalone and move it later.
    ByTemporary {
        workspace_id: WorkspaceId,
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
