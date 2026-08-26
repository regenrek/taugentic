use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{DaemonEvent, DaemonEventKind, PublicDaemonEvent, u64_string};

/// Durable paging cursor for `daemon.activity.page`.
///
/// This is session-scoped durable paging only. It is not the live resume cursor
/// used by `daemon.subscribe`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ActivityCursor {
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ActivityPageItem {
    pub cursor: ActivityCursor,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub occurred_at_ms: u64,
    pub event: DaemonEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct PublicActivityPageItem {
    pub cursor: ActivityCursor,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub occurred_at_ms: u64,
    pub event: PublicDaemonEvent,
}

impl From<ActivityPageItem> for PublicActivityPageItem {
    fn from(value: ActivityPageItem) -> Self {
        Self {
            cursor: value.cursor,
            occurred_at_ms: value.occurred_at_ms,
            event: value.event.redact_for_public(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ActivityPageQuery {
    pub limit: u32,
    /// Durable paging cursor for older activity items from `daemon.activity.page`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<ActivityCursor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<DaemonEventKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct ActivityPageResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ActivityPageItem>,
    /// Cursor for fetching older durable activity items with `daemon.activity.page`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_before: Option<ActivityCursor>,
    /// Latest durable activity cursor from the canonical event log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_activity_cursor: Option<ActivityCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct PublicActivityPageResult {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<PublicActivityPageItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_before: Option<ActivityCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_activity_cursor: Option<ActivityCursor>,
}

impl From<ActivityPageResult> for PublicActivityPageResult {
    fn from(value: ActivityPageResult) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            next_before: value.next_before,
            latest_activity_cursor: value.latest_activity_cursor,
        }
    }
}
