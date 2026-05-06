use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{PublicDaemonEventEnvelope, RunSummary, SessionSummary, u64_string};

pub const DEFAULT_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT: u32 = 8;
pub const MAX_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT: u32 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export_to = "generated/")]
pub struct SessionOverviewQuery {
    pub recent_activity_limit: u32,
}

impl Default for SessionOverviewQuery {
    fn default() -> Self {
        Self {
            recent_activity_limit: DEFAULT_SESSION_OVERVIEW_RECENT_ACTIVITY_LIMIT,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ApprovalAttentionState {
    Idle,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum SessionOverviewLaneStatus {
    Idle,
    Active,
    WaitingForApproval,
    Failed,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SessionOverview {
    pub session: SessionSummary,
    /// Most recent run summary for this session, if one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_run: Option<RunSummary>,
    /// Daemon-owned lane projection for operator-facing session/run state.
    pub lane_status: SessionOverviewLaneStatus,
    /// True when the session currently owns active or waiting work.
    pub is_active: bool,
    /// Approval attention state owned by the daemon read model.
    pub approval_attention: ApprovalAttentionState,
    /// Count of approvals currently awaiting a decision for this session.
    pub pending_approval_count: u32,
    /// Timestamp of the newest daemon-owned activity item for this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub last_activity_at_ms: Option<u64>,
    /// Compact daemon-owned preview of the newest activity item for this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event_preview: Option<String>,
    /// Recent public daemon activity for this session, ordered newest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_activity: Vec<PublicDaemonEventEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SessionOverviewResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<SessionOverview>,
}
