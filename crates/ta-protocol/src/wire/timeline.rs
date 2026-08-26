use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentToolCallOutcome, ApprovalDecision, ApprovalId, ApprovalScope, ArtifactKind, BudgetMetric,
    BudgetScope, ConflictWarning, OutputContractKind, ReceiptId, ReceiptKind, ReceiptState, RunId,
    RunStatus, SessionId, TokenUsageRecordedEvent, WorktreeInfo, u64_string,
};

pub const RUN_TIMELINE_EVENT_DEFAULT_LIMIT: u32 = 500;
pub const RUN_TIMELINE_EVENT_MAX_LIMIT: u32 = 2_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct GetRunTimelineQuery {
    pub session_id: SessionId,
    pub root_run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub after_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunTimeline {
    pub session_id: SessionId,
    pub root_run_id: RunId,
    pub runs: Vec<RunTimelineRun>,
    pub events: Vec<RunTimelineEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub latest_event_seq: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunTimelineRun {
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<RunId>,
    pub depth: u32,
    pub status: RunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<OutputContractKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "string | null")]
    pub ended_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_info: Option<WorktreeInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claimed_files: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunTimelineEventKind {
    RunStatus,
    ApprovalRequested,
    ApprovalResolved,
    ClaimConflict,
    BudgetExceeded,
    TokenUsage,
    ToolCall,
    Artifact,
    Receipt,
    AgentStream,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct RunTimelineEvent {
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub seq: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub occurred_at_ms: u64,
    pub run_id: RunId,
    pub kind: RunTimelineEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<RunStatus>,
    pub label: String,
    pub payload: RunTimelineEventPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum RunTimelineEventPayload {
    Run {
        detail: String,
    },
    ApprovalRequested {
        #[serde(rename = "approvalId")]
        #[ts(rename = "approvalId")]
        approval_id: ApprovalId,
        scope: ApprovalScope,
    },
    ApprovalResolved {
        #[serde(rename = "approvalId")]
        #[ts(rename = "approvalId")]
        approval_id: ApprovalId,
        decision: ApprovalDecision,
    },
    Conflict {
        warning: ConflictWarning,
    },
    BudgetExceeded {
        scope: BudgetScope,
        metric: BudgetMetric,
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(type = "string")]
        limit: u64,
        #[serde(with = "u64_string")]
        #[schemars(schema_with = "u64_string::json_schema")]
        #[ts(type = "string")]
        actual: u64,
    },
    TokenUsage {
        usage: TokenUsageRecordedEvent,
    },
    ToolCall {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[serde(rename = "toolName")]
        #[ts(rename = "toolName")]
        tool_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        outcome: Option<AgentToolCallOutcome>,
    },
    Artifact {
        #[serde(rename = "artifactId")]
        #[ts(rename = "artifactId")]
        artifact_id: String,
        #[serde(rename = "artifactKind")]
        #[ts(rename = "artifactKind")]
        artifact_kind: ArtifactKind,
    },
    Receipt {
        #[serde(rename = "receiptId")]
        #[ts(rename = "receiptId")]
        receipt_id: ReceiptId,
        #[serde(rename = "receiptKind")]
        #[ts(rename = "receiptKind")]
        receipt_kind: ReceiptKind,
        #[serde(rename = "receiptState")]
        #[ts(rename = "receiptState")]
        receipt_state: ReceiptState,
    },
    AgentStream {
        #[serde(rename = "frameKind")]
        #[ts(rename = "frameKind")]
        frame_kind: String,
    },
}
