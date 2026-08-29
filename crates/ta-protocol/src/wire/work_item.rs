use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{CodeHostAccountId, RunSummary, u64_string};

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(transparent)]
#[ts(export_to = "generated/")]
pub struct WorkItemKey(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum WorkSourceKind {
    GitHub,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum WorkSource {
    GitHub {
        repo_owner: String,
        repo_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum WorkItemStatus {
    Available,
    Dismissed,
    Triggered,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkItem {
    pub key: WorkItemKey,
    pub source: WorkSource,
    pub external_id: String,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub url: String,
    #[ts(type = "number")]
    pub fetched_at_ms: u64,
    pub status: WorkItemStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triggered_run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkSourceConfig {
    pub repositories: Vec<GitHubWorkSourceRepository>,
    pub label_filter: WorkSourceLabelFilter,
    pub recipe_mappings: Vec<WorkSourceRecipeMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct GitHubWorkSourceRepository {
    pub account_id: CodeHostAccountId,
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum WorkSourceLabelFilter {
    Any,
    All(Vec<String>),
    AnyOf(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkSourceRecipeMapping {
    pub label: String,
    pub recipe_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct SourceCursor {
    pub etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number | null")]
    pub last_fetched_at_ms: Option<u64>,
}

impl WorkItemKey {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err("work item key is empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn github(repo_owner: &str, repo_name: &str, external_id: &str) -> Result<Self, String> {
        Self::new(format!(
            "github:{}/{}#{}",
            require_key_part("repo owner", repo_owner)?,
            require_key_part("repo name", repo_name)?,
            require_key_part("external id", external_id)?
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl WorkSourceLabelFilter {
    pub fn matches(&self, labels: &[String]) -> bool {
        match self {
            Self::Any => true,
            Self::All(required) => required.iter().all(|label| has_label(labels, label)),
            Self::AnyOf(allowed) => {
                allowed.is_empty() || allowed.iter().any(|label| has_label(labels, label))
            }
        }
    }
}

impl SourceCursor {
    pub fn empty() -> Self {
        Self {
            etag: None,
            last_fetched_at_ms: None,
        }
    }
}

fn has_label(labels: &[String], expected: &str) -> bool {
    labels
        .iter()
        .any(|label| label.eq_ignore_ascii_case(expected.trim()))
}

fn require_key_part<'a>(name: &str, value: &'a str) -> Result<&'a str, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{name} is empty"));
    }
    Ok(value)
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export_to = "generated/")]
pub struct WorkItemListQuery {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkItemListResult {
    pub items: Vec<WorkItem>,
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
    #[ts(type = "string | null")]
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
    pub key: WorkItemKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkItemDismissResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<WorkItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkItemTriggerParams {
    pub key: WorkItemKey,
    pub selection: crate::wire::AgentRuntimeSelection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct WorkItemTriggerResult {
    pub item: WorkItem,
    pub run: RunSummary,
}
