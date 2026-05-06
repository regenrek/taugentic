use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{RunSummary, u64_string};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export_to = "generated/")]
pub struct WorkItemListQuery {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkItemListResult {
    pub items: Vec<ta_work_source::WorkItem>,
    pub sync: WorkSourceSyncStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkSourceSyncStatus {
    pub state: WorkSourceSyncState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(with = "u64_string::option")]
    #[schemars(schema_with = "u64_string::option::json_schema")]
    #[ts(type = "bigint | null")]
    pub last_fetched_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum WorkSourceSyncState {
    Disabled,
    Idle,
    RefreshQueued,
    Refreshing,
    RateLimited,
    Error,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export_to = "generated/")]
pub struct WorkItemRefreshParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkItemDismissParams {
    pub key: ta_work_source::WorkItemKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkItemDismissResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<ta_work_source::WorkItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkItemTriggerParams {
    pub key: ta_work_source::WorkItemKey,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkItemTriggerResult {
    pub item: ta_work_source::WorkItem,
    pub run: RunSummary,
}
