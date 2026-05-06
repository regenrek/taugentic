use serde::{Deserialize, Serialize};
use ta_protocol::wire::{ArtifactId, ArtifactKind, RunId, SessionId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub id: ArtifactId,
    pub session_id: SessionId,
    pub run_id: RunId,
    pub kind: ArtifactKind,
    pub storage_path: String,
}
