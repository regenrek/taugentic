use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{ProjectId, WorkspaceId, identifier, u64_string};

identifier!(TerminalSessionId, "terminal session");

pub const TERMINAL_MIN_ROWS: u16 = 2;
pub const TERMINAL_MAX_ROWS: u16 = 512;
pub const TERMINAL_MIN_COLS: u16 = 2;
pub const TERMINAL_MAX_COLS: u16 = 1_024;
pub const TERMINAL_INPUT_MAX_BYTES: usize = 64 * 1024;
pub const TERMINAL_OUTPUT_CHUNK_MAX_BYTES: usize = 64 * 1024;
pub const TERMINAL_SNAPSHOT_MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum TerminalSessionStatus {
    Running,
    Exited,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalSessionSummary {
    pub id: TerminalSessionId,
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub status: TerminalSessionStatus,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalSpawnParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub rows: u16,
    pub cols: u16,
    pub user_approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalSpawnResult {
    pub terminal: TerminalSessionSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalListParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalListResult {
    pub terminals: Vec<TerminalSessionSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalAttachParams {
    pub terminal_id: TerminalSessionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalAttachResult {
    pub terminal: TerminalSessionSummary,
    pub snapshot_base64: String,
    pub snapshot_truncated: bool,
    #[serde(with = "u64_string")]
    #[schemars(with = "String")]
    #[ts(type = "string")]
    pub latest_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalInputParams {
    pub terminal_id: TerminalSessionId,
    pub data_base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalInputResult {
    pub accepted_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalResizeParams {
    pub terminal_id: TerminalSessionId,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalResizeResult {
    pub terminal: TerminalSessionSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalDetachParams {
    pub terminal_id: TerminalSessionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalDetachResult {
    pub detached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalCloseParams {
    pub terminal_id: TerminalSessionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalCloseResult {
    pub terminal: TerminalSessionSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export_to = "generated/")]
pub enum TerminalStreamEvent {
    Output {
        #[serde(with = "u64_string")]
        #[schemars(with = "String")]
        #[ts(type = "string")]
        sequence: u64,
        data_base64: String,
    },
    Exited,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct TerminalEventParams {
    pub terminal_id: TerminalSessionId,
    pub event: TerminalStreamEvent,
}
