use serde::{Deserialize, Serialize};
use ta_protocol::wire::{GitCheckpointPhase, RunId, WorkspaceId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRecord {
    pub checkpoint_id: String,
    pub workspace_id: WorkspaceId,
    pub run_id: RunId,
    pub revision: u64,
    pub phase: GitCheckpointPhase,
    pub base_head: Option<String>,
    pub staged_commit: String,
    pub full_commit: String,
    pub fingerprint: String,
    pub created_at_ms: u64,
}
