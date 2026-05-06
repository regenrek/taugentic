use serde::{Deserialize, Serialize};
use ta_protocol::wire::RunId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRecord {
    pub run_id: RunId,
    pub revision: u64,
    pub artifact_path: String,
}
