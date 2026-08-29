use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{ProjectId, WorkspaceId, identifier, u64_string};

pub const CODE_HOST_PAGE_DEFAULT_LIMIT: u32 = 50;
pub const CODE_HOST_PAGE_MAX_LIMIT: u32 = 100;
pub const CODE_HOST_TEXT_MAX_BYTES: usize = 256 * 1024;
pub const CODE_HOST_RESPONSE_MAX_BYTES: usize = 4 * 1024 * 1024;

identifier!(CodeHostAccountId, "code host account");
identifier!(CodeHostPullRequestId, "code host pull request");

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum CodeHostProviderKind {
    GitHub,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostAccount {
    pub id: CodeHostAccountId,
    pub provider: CodeHostProviderKind,
    pub display_name: String,
    pub account_login: String,
    pub host: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostAccountListParams {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostAccountListResult {
    pub accounts: Vec<CodeHostAccount>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostAccountConnectParams {
    pub provider: CodeHostProviderKind,
    pub display_name: String,
    pub host: String,
    pub access_token: String,
}

impl std::fmt::Debug for CodeHostAccountConnectParams {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodeHostAccountConnectParams")
            .field("provider", &self.provider)
            .field("display_name", &self.display_name)
            .field("host", &self.host)
            .field("access_token", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostAccountConnectResult {
    pub account: CodeHostAccount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostAccountDisconnectParams {
    pub account_id: CodeHostAccountId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostAccountDisconnectResult {
    pub disconnected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostRepositoryRef {
    pub provider: CodeHostProviderKind,
    pub host: String,
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostRemote {
    pub remote_name: String,
    pub repository: CodeHostRepositoryRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostRepositoryContextParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostRepositoryContextResult {
    pub remotes: Vec<CodeHostRemote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostCommitSummary {
    pub id: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostPushPrepareParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub account_id: CodeHostAccountId,
    pub remote_name: String,
    pub destination_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostPushPrepareResult {
    pub token: String,
    pub remote: CodeHostRemote,
    pub source_head: String,
    pub destination_branch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_head: Option<String>,
    pub commits: Vec<CodeHostCommitSummary>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostPushApplyParams {
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostPushApplyResult {
    pub remote: CodeHostRemote,
    pub source_head: String,
    pub destination_branch: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum CodeHostPullRequestState {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestSummary {
    pub id: CodeHostPullRequestId,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub number: u64,
    pub title: String,
    pub state: CodeHostPullRequestState,
    pub draft: bool,
    pub author_login: String,
    pub head_repository: CodeHostRepositoryRef,
    pub head_branch: String,
    pub head_sha: String,
    pub base_repository: CodeHostRepositoryRef,
    pub base_branch: String,
    pub web_url: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestDetail {
    pub summary: CodeHostPullRequestSummary,
    pub body: String,
    pub mergeable: Option<bool>,
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostPage {
    pub items: Vec<CodeHostPullRequestSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestListParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub account_id: CodeHostAccountId,
    pub repository: CodeHostRepositoryRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestDetailParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub account_id: CodeHostAccountId,
    pub repository: CodeHostRepositoryRef,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub number: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestEnsureParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub account_id: CodeHostAccountId,
    pub head_remote_name: String,
    pub head_branch: String,
    pub base_remote_name: String,
    pub base_branch: String,
    pub title: String,
    pub body: String,
    pub draft: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestEnsureResult {
    pub pull_request: CodeHostPullRequestSummary,
    pub created: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum CodeHostCheckStatus {
    Queued,
    InProgress,
    Completed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostCheck {
    pub id: String,
    pub name: String,
    pub status: CodeHostCheckStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestChecksResult {
    pub checks: Vec<CodeHostCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestChecksParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub account_id: CodeHostAccountId,
    pub repository: CodeHostRepositoryRef,
    pub head_sha: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum CodeHostCommentKind {
    Conversation,
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostComment {
    pub id: String,
    pub kind: CodeHostCommentKind,
    pub author_login: String,
    pub body: String,
    pub web_url: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostReview {
    pub id: String,
    pub author_login: String,
    pub state: String,
    pub body: String,
    pub web_url: String,
    pub submitted_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostTimelineItem {
    pub id: String,
    pub kind: String,
    pub actor_login: String,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestActivityResult {
    pub comments: Vec<CodeHostComment>,
    pub reviews: Vec<CodeHostReview>,
    pub timeline: Vec<CodeHostTimelineItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestActivityParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub account_id: CodeHostAccountId,
    pub repository: CodeHostRepositoryRef,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub number: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestCommentCreateParams {
    pub project_id: ProjectId,
    pub workspace_id: WorkspaceId,
    pub account_id: CodeHostAccountId,
    pub repository: CodeHostRepositoryRef,
    #[serde(with = "u64_string")]
    #[schemars(schema_with = "u64_string::json_schema")]
    #[ts(type = "string")]
    pub number: u64,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub struct CodeHostPullRequestCommentCreateResult {
    pub comment: CodeHostComment,
}
