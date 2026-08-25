use std::{fmt, sync::Arc};

use reqwest::{StatusCode, header};
use serde::Deserialize;
use ta_host_platform::{HostSecretKey, HostSecretStore};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{
    FetchOutcome, SourceCursor, WorkItem, WorkItemKey, WorkItemStatus, WorkSource, WorkSourceError,
    WorkSourceLabelFilter,
    error::{RateLimitBackoff, RateLimitReason, rate_limit_backoff},
};

const GITHUB_API_BASE_URL: &str = "https://api.github.com";
const GITHUB_API_VERSION: &str = "2022-11-28";
const GITHUB_ACCEPT: &str = "application/vnd.github+json";
const DEFAULT_PAGE_SIZE: u8 = 100;
const DEFAULT_MAX_PAGES: u8 = 10;
const SECONDARY_RATE_LIMIT_BACKOFF_SECS: u64 = 60;
const GITHUB_SECRET_SERVICE_NAME: &str = "taugentic.host.secrets";
const GITHUB_PAT_SECRET_KEY: &str = "work_source.github/github_pat";

#[derive(Clone)]
pub struct GitHubToken(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubProviderConfig {
    pub repo_owner: String,
    pub repo_name: String,
    pub label_filter: WorkSourceLabelFilter,
    pub max_pages: u8,
    pub base_url: String,
}

#[derive(Clone)]
pub struct GitHubIssueProvider {
    client: reqwest::Client,
    config: GitHubProviderConfig,
}

pub trait GitHubCredentialProvider: Send + Sync {
    fn token(&self) -> Result<GitHubToken, WorkSourceError>;
}

pub struct HostSecretsGitHubCredentialProvider {
    store: Arc<dyn HostSecretStore>,
    key: HostSecretKey,
}

impl HostSecretsGitHubCredentialProvider {
    pub fn new(store: Arc<dyn HostSecretStore>) -> Result<Self, WorkSourceError> {
        let key = HostSecretKey::new(GITHUB_PAT_SECRET_KEY).map_err(map_host_secret_error)?;
        Ok(Self { store, key })
    }

    pub fn from_default_store() -> Result<Self, WorkSourceError> {
        let store = ta_host_platform::default_host_secret_store(GITHUB_SECRET_SERVICE_NAME)
            .map_err(map_host_secret_error)?;
        Self::new(store)
    }
}

#[derive(Debug, Deserialize)]
struct GitHubIssue {
    number: u64,
    title: String,
    body: Option<String>,
    html_url: Option<String>,
    labels: Vec<GitHubIssueLabel>,
    pull_request: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct GitHubIssueLabel {
    name: Option<String>,
}

impl GitHubToken {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkSourceError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(WorkSourceError::CredentialsMissing);
        }
        Ok(Self(value))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GitHubToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitHubToken(REDACTED)")
    }
}

impl GitHubCredentialProvider for HostSecretsGitHubCredentialProvider {
    fn token(&self) -> Result<GitHubToken, WorkSourceError> {
        self.store
            .load_secret(&self.key)
            .map_err(map_host_secret_error)?
            .map(|secret| GitHubToken::new(secret.expose_secret().to_string()))
            .transpose()?
            .ok_or(WorkSourceError::CredentialsMissing)
    }
}

impl GitHubProviderConfig {
    pub fn new(
        repo_owner: impl Into<String>,
        repo_name: impl Into<String>,
        label_filter: WorkSourceLabelFilter,
    ) -> Result<Self, WorkSourceError> {
        let repo_owner = normalize_required("repo owner", repo_owner.into())?;
        let repo_name = normalize_required("repo name", repo_name.into())?;
        Ok(Self {
            repo_owner,
            repo_name,
            label_filter,
            max_pages: DEFAULT_MAX_PAGES,
            base_url: GITHUB_API_BASE_URL.to_string(),
        })
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Result<Self, WorkSourceError> {
        let base_url = normalize_required("base url", base_url.into())?;
        Url::parse(&base_url).map_err(|error| WorkSourceError::InvalidConfig(error.to_string()))?;
        self.base_url = base_url.trim_end_matches('/').to_string();
        Ok(self)
    }

    pub fn with_max_pages(mut self, max_pages: u8) -> Result<Self, WorkSourceError> {
        if max_pages == 0 {
            return Err(WorkSourceError::InvalidConfig(
                "GitHub max_pages must be greater than zero".to_string(),
            ));
        }
        self.max_pages = max_pages;
        Ok(self)
    }
}

impl GitHubIssueProvider {
    pub fn new(config: GitHubProviderConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    pub async fn fetch(
        &self,
        token: &GitHubToken,
        cursor: SourceCursor,
        fetched_at_ms: u64,
        cancellation: CancellationToken,
    ) -> Result<FetchOutcome, WorkSourceError> {
        if cancellation.is_cancelled() {
            return Err(WorkSourceError::Cancelled);
        }

        let mut all_items = Vec::new();
        let mut next_etag = cursor.etag.clone();
        for page in 1..=self.config.max_pages {
            let page_result = self
                .fetch_page(
                    token,
                    cursor.etag.as_deref().filter(|_| page == 1),
                    page,
                    &cancellation,
                )
                .await?;
            match page_result {
                PageFetchOutcome::NotModified => {
                    return Ok(FetchOutcome::NotModified {
                        cursor: SourceCursor {
                            etag: cursor.etag,
                            last_fetched_at_ms: Some(fetched_at_ms),
                        },
                    });
                }
                PageFetchOutcome::Items { issues, etag } => {
                    if page == 1 {
                        next_etag = etag.or(next_etag);
                    }
                    let page_len = issues.len();
                    all_items.extend(self.items_from_issues(issues, fetched_at_ms)?);
                    if page_len < usize::from(DEFAULT_PAGE_SIZE) {
                        break;
                    }
                }
            }
        }

        Ok(FetchOutcome::Items {
            items: all_items,
            cursor: SourceCursor {
                etag: next_etag,
                last_fetched_at_ms: Some(fetched_at_ms),
            },
        })
    }

    async fn fetch_page(
        &self,
        token: &GitHubToken,
        etag: Option<&str>,
        page: u8,
        cancellation: &CancellationToken,
    ) -> Result<PageFetchOutcome, WorkSourceError> {
        let url = self.page_url(page)?;
        let mut request = self
            .client
            .get(url)
            .bearer_auth(token.as_str())
            .header(header::ACCEPT, GITHUB_ACCEPT)
            .header("x-github-api-version", GITHUB_API_VERSION);
        if let Some(etag) = etag {
            request = request.header(header::IF_NONE_MATCH, etag);
        }
        let response = send_with_cancellation(request, cancellation).await?;
        read_page_response(response).await
    }

    fn page_url(&self, page: u8) -> Result<Url, WorkSourceError> {
        let mut url = Url::parse(&self.config.base_url)
            .map_err(|error| WorkSourceError::InvalidConfig(error.to_string()))?;
        url.path_segments_mut()
            .map_err(|_| {
                WorkSourceError::InvalidConfig("GitHub base URL cannot be a base".to_string())
            })?
            .extend([
                "repos",
                &self.config.repo_owner,
                &self.config.repo_name,
                "issues",
            ]);
        url.query_pairs_mut()
            .append_pair("state", "open")
            .append_pair("per_page", &DEFAULT_PAGE_SIZE.to_string())
            .append_pair("page", &page.to_string());
        Ok(url)
    }

    fn items_from_issues(
        &self,
        issues: Vec<GitHubIssue>,
        fetched_at_ms: u64,
    ) -> Result<Vec<WorkItem>, WorkSourceError> {
        issues
            .into_iter()
            .filter(|issue| issue.pull_request.is_none())
            .map(|issue| self.item_from_issue(issue, fetched_at_ms))
            .filter_map(|item| match item {
                Ok(item) if self.config.label_filter.matches(&item.labels) => Some(Ok(item)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    fn item_from_issue(
        &self,
        issue: GitHubIssue,
        fetched_at_ms: u64,
    ) -> Result<WorkItem, WorkSourceError> {
        let external_id = issue.number.to_string();
        let labels = issue
            .labels
            .into_iter()
            .filter_map(|label| label.name)
            .filter(|label| !label.trim().is_empty())
            .collect::<Vec<_>>();
        let key = WorkItemKey::github(
            &self.config.repo_owner,
            &self.config.repo_name,
            &external_id,
        )
        .map_err(WorkSourceError::InvalidResponse)?;
        Ok(WorkItem {
            key,
            source: WorkSource::GitHub {
                repo_owner: self.config.repo_owner.clone(),
                repo_name: self.config.repo_name.clone(),
            },
            external_id,
            title: issue.title,
            body: issue.body.unwrap_or_default(),
            labels,
            url: issue.html_url.unwrap_or_else(|| {
                format!(
                    "https://github.com/{}/{}/issues/{}",
                    self.config.repo_owner, self.config.repo_name, issue.number
                )
            }),
            fetched_at_ms,
            status: WorkItemStatus::Available,
            triggered_run_id: None,
        })
    }
}

enum PageFetchOutcome {
    Items {
        issues: Vec<GitHubIssue>,
        etag: Option<String>,
    },
    NotModified,
}

async fn send_with_cancellation(
    request: reqwest::RequestBuilder,
    cancellation: &CancellationToken,
) -> Result<reqwest::Response, WorkSourceError> {
    tokio::select! {
        _ = cancellation.cancelled() => Err(WorkSourceError::Cancelled),
        response = request.send() => response.map_err(WorkSourceError::Network),
    }
}

async fn read_page_response(
    response: reqwest::Response,
) -> Result<PageFetchOutcome, WorkSourceError> {
    let status = response.status();
    if status == StatusCode::NOT_MODIFIED {
        return Ok(PageFetchOutcome::NotModified);
    }
    let headers = response.headers().clone();
    let etag = headers
        .get(header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response.text().await.map_err(WorkSourceError::Network)?;
    if !status.is_success() {
        return Err(WorkSourceError::HttpStatus {
            status,
            backoff: status_backoff(status, &headers, &body),
        });
    }
    let issues = serde_json::from_str::<Vec<GitHubIssue>>(&body)
        .map_err(|error| WorkSourceError::InvalidResponse(error.to_string()))?;
    Ok(PageFetchOutcome::Items { issues, etag })
}

fn status_backoff(
    status: StatusCode,
    headers: &header::HeaderMap,
    body: &str,
) -> Option<RateLimitBackoff> {
    rate_limit_backoff(headers).or_else(|| {
        let body = body.to_ascii_lowercase();
        let secondary = status == StatusCode::FORBIDDEN
            && (body.contains("secondary rate limit") || body.contains("abuse detection"));
        if secondary || status == StatusCode::TOO_MANY_REQUESTS {
            return Some(RateLimitBackoff {
                retry_after: std::time::Duration::from_secs(SECONDARY_RATE_LIMIT_BACKOFF_SECS),
                reason: RateLimitReason::Secondary,
            });
        }
        None
    })
}

fn normalize_required(name: &'static str, value: String) -> Result<String, WorkSourceError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(WorkSourceError::InvalidConfig(format!("{name} is empty")));
    }
    Ok(value)
}

fn map_host_secret_error(error: ta_host_platform::HostSecretError) -> WorkSourceError {
    match error {
        ta_host_platform::HostSecretError::NotFound => WorkSourceError::CredentialsMissing,
        error => WorkSourceError::CredentialsBackend(error.to_string()),
    }
}

#[cfg(test)]
mod tests;
