use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{AgentStreamTurnId, ArtifactId, RunId, SessionId, u64_string};

pub type ReceiptId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ContextReceipt {
    pub id: ReceiptId,
    pub session_id: SessionId,
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<RunId>,
    pub kind: ReceiptKind,
    pub provenance: ReceiptProvenance,
    pub state: ReceiptState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "bigint")]
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub promoted_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub quarantined_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ReceiptKind {
    Evidence,
    Patch,
    TestOutput,
    ReviewFinding,
    Artifact,
    Risk,
    Blocker,
    Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ReceiptState {
    Returned,
    Promoted,
    Quarantined,
}

/// Provenance shape rules:
/// - artifact-derived: only `artifact_id` is set; identity = (session, run, kind, artifact_id).
/// - event-derived: both `event_seq` and `agent_turn_id` are set; identity = (session, run, kind, event_seq, agent_turn_id).
/// - free-form: all identifying fields are None.
///
/// `stream_cursor` is descriptive metadata (e.g. for UI navigation) and may be
/// present in any shape. It is never part of the unique identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ReceiptProvenance {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_turn_id: Option<AgentStreamTurnId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub event_seq: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_cursor: Option<String>,
}
