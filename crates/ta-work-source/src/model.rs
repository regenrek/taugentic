use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(transparent)]
#[ts(export_to = "generated/")]
pub struct WorkItemKey(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
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
    pub repositories: Vec<GitHubRepository>,
    pub label_filter: WorkSourceLabelFilter,
    pub recipe_mappings: Vec<WorkSourceRecipeMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct GitHubRepository {
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

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn github_key_is_stable() -> Result<(), Box<dyn Error>> {
        let key = WorkItemKey::github("regenrek", "taugentic", "42")?;
        assert_eq!(key.as_str(), "github:regenrek/taugentic#42");
        Ok(())
    }

    #[test]
    fn label_filter_is_case_insensitive() {
        let labels = vec!["Taugentic".to_string(), "Ready".to_string()];
        assert!(WorkSourceLabelFilter::All(vec!["ready".to_string()]).matches(&labels));
        assert!(
            WorkSourceLabelFilter::AnyOf(vec!["blocked".to_string(), "taugentic".to_string()])
                .matches(&labels)
        );
        assert!(!WorkSourceLabelFilter::All(vec!["missing".to_string()]).matches(&labels));
    }

    #[test]
    fn work_item_timestamp_remains_a_numeric_domain_value() {
        let item = WorkItem {
            key: WorkItemKey::new("github:owner/repo#1").expect("work item key"),
            source: WorkSource::GitHub {
                repo_owner: "owner".to_string(),
                repo_name: "repo".to_string(),
            },
            external_id: "1".to_string(),
            title: "title".to_string(),
            body: String::new(),
            labels: Vec::new(),
            url: "https://example.invalid/1".to_string(),
            fetched_at_ms: 1_725_000_000_000,
            status: WorkItemStatus::Available,
            triggered_run_id: None,
        };

        let json = serde_json::to_value(&item).expect("work item should serialize");
        assert_eq!(json["fetchedAtMs"], 1_725_000_000_000_u64);
        assert_eq!(
            serde_json::from_value::<WorkItem>(json)
                .expect("numeric work item should deserialize")
                .fetched_at_ms,
            item.fetched_at_ms
        );
    }
}
