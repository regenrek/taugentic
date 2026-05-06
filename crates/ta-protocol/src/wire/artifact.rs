use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{ArtifactId, DaemonEventCursor, RunId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export_to = "generated/")]
pub enum ArtifactKind {
    Transcript,
    Patch,
    FileSnapshot,
    CommandLog,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ArtifactSummary {
    pub id: ArtifactId,
    pub run_id: RunId,
    pub kind: ArtifactKind,
    pub storage_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ArtifactSnapshotResult {
    pub items: Vec<ArtifactSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_cursor: Option<DaemonEventCursor>,
}
