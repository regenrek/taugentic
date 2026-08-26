use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::WorkItem;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SourceCursor {
    pub etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_cursor_timestamp_remains_a_numeric_domain_value() {
        let cursor = SourceCursor {
            etag: Some("etag".to_string()),
            last_fetched_at_ms: Some(1_725_000_000_000),
        };

        let json = serde_json::to_value(&cursor).expect("source cursor should serialize");
        assert_eq!(json["lastFetchedAtMs"], 1_725_000_000_000_u64);
        assert_eq!(
            serde_json::from_value::<SourceCursor>(json)
                .expect("numeric source cursor should deserialize")
                .last_fetched_at_ms,
            cursor.last_fetched_at_ms
        );
    }
}
