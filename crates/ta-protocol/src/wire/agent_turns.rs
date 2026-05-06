use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{
    ActivityCursor, AgentStreamItemId, AgentStreamTurnId, AgentToolCallOutcome, DaemonEventCursor,
    RunId, RuntimeLanePendingState, SessionId, u64_string,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentTurnsPageQuery {
    pub limit: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<ActivityCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentAssistantRow {
    pub cursor: ActivityCursor,
    pub session_id: SessionId,
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<AgentStreamTurnId>,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(as = "u64")]
    pub started_at_ms: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(as = "u64")]
    pub completed_at_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentToolCallRow {
    pub cursor: ActivityCursor,
    pub session_id: SessionId,
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<AgentStreamTurnId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<AgentStreamItemId>,
    pub tool_name: String,
    pub input: String,
    pub output: String,
    pub outcome: AgentToolCallOutcome,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(as = "u64")]
    pub started_at_ms: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(as = "u64")]
    pub completed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentPendingStateRow {
    pub cursor: ActivityCursor,
    pub session_id: SessionId,
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<AgentStreamTurnId>,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(as = "u64")]
    pub occurred_at_ms: u64,
    pub state: RuntimeLanePendingState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum AgentTurnRow {
    Assistant(AgentAssistantRow),
    ToolCall(AgentToolCallRow),
    PendingState(AgentPendingStateRow),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct AgentTurnsPageResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<AgentTurnRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_before: Option<ActivityCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_cursor: Option<DaemonEventCursor>,
}
