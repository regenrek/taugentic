use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{ActivityCursor, RunId, SessionId, u64_string};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ThreadWorkspaceQuery {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ThreadWorkspaceUpdateCommand {
    pub mutation: ThreadWorkspaceMutation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub enum ThreadWorkspaceMutation {
    GoalSet { value: String },
    PlanSet { value: String },
    NotesSet { value: String },
    RecapSet { value: String },
    PinAdded { pin: ThreadWorkspacePin },
    PinRemoved { cursor: ActivityCursor },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ThreadWorkspacePin {
    pub run_id: RunId,
    pub cursor: ActivityCursor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ThreadWorkspaceWorkLogEntry {
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub sequence: u64,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub occurred_at_ms: u64,
    pub kind: ThreadWorkspaceWorkLogKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub enum ThreadWorkspaceWorkLogKind {
    GoalSet,
    PlanSet,
    NotesSet,
    RecapSet,
    PinAdded,
    PinRemoved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ThreadWorkspaceResult {
    pub session_id: SessionId,
    pub goal: String,
    pub plan: String,
    pub notes: String,
    pub recap: String,
    pub pins: Vec<ThreadWorkspacePin>,
    pub work_log: Vec<ThreadWorkspaceWorkLogEntry>,
}
