use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::BrowserProfileId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct BrowserProfile {
    pub id: BrowserProfileId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum BrowserNavigationKind {
    Navigate,
    Back,
    Forward,
    Reload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct BrowserNavigationRequest {
    pub kind: BrowserNavigationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum BrowserActionKind {
    NavigationIntent,
    NavigationAction,
    NavigationResponse,
    DownloadDestination,
}

/// Exact native BrowserSurface action families. The daemon owns the decision
/// for each family; downloadDestination never shares navigation semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct BrowserActionRequest {
    pub request_id: String,
    pub profile_id: BrowserProfileId,
    pub kind: BrowserActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<BrowserNavigationRequest>,
    /// Present only for a native navigationAction. A missing value must be
    /// cancelled by policy rather than guessed by a desktop client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub should_perform_download: Option<bool>,
    /// Present only for a native navigationResponse. A missing value must be
    /// cancelled by policy rather than guessed by a desktop client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub can_show_mime_type: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum BrowserActionDecision {
    Allow,
    Cancel,
    Download,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct BrowserActionResult {
    pub request_id: String,
    pub decision: BrowserActionDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct BrowserProfileRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct BrowserProfileResult {
    pub profile: BrowserProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct BrowserClearDataRequest {
    pub request_id: String,
    pub profile_id: BrowserProfileId,
}
