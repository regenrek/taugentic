use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::WorkItem;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SourceCursor {
    pub etag: Option<String>,
    pub last_fetched_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchOutcome {
    Items {
        items: Vec<WorkItem>,
        cursor: SourceCursor,
    },
    NotModified {
        cursor: SourceCursor,
    },
}

impl SourceCursor {
    pub fn empty() -> Self {
        Self {
            etag: None,
            last_fetched_at_ms: None,
        }
    }
}
