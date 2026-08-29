use ta_code_host::{CodeHostAccessToken, CodeHostError, GitHubClient, GitHubIssue};
use ta_protocol::wire::{
    CodeHostProviderKind, CodeHostRepositoryRef, SourceCursor, WorkItem, WorkItemKey,
    WorkItemStatus, WorkSource, WorkSourceLabelFilter,
};
use tokio_util::sync::CancellationToken;

use crate::{FetchOutcome, WorkSourceError};

const DEFAULT_MAX_PAGES: u8 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubProviderConfig {
    pub repository: CodeHostRepositoryRef,
    pub label_filter: WorkSourceLabelFilter,
    pub max_pages: u8,
}

#[derive(Clone)]
pub struct GitHubIssueProvider {
    client: GitHubClient,
    config: GitHubProviderConfig,
}

impl GitHubProviderConfig {
    pub fn github_dot_com(
        repo_owner: impl Into<String>,
        repo_name: impl Into<String>,
        label_filter: WorkSourceLabelFilter,
    ) -> Result<Self, WorkSourceError> {
        let owner = normalize_required("repo owner", repo_owner.into())?;
        let name = normalize_required("repo name", repo_name.into())?;
        Ok(Self {
            repository: CodeHostRepositoryRef {
                provider: CodeHostProviderKind::GitHub,
                host: "github.com".to_string(),
                owner,
                name,
            },
            label_filter,
            max_pages: DEFAULT_MAX_PAGES,
        })
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
    pub fn new(client: GitHubClient, config: GitHubProviderConfig) -> Self {
        Self { client, config }
    }

    pub async fn fetch(
        &self,
        token: &CodeHostAccessToken,
        cursor: SourceCursor,
        fetched_at_ms: u64,
        cancellation: CancellationToken,
    ) -> Result<FetchOutcome, WorkSourceError> {
        let mut all_items = Vec::new();
        let mut etag = cursor.etag.clone();
        let mut page_cursor = None;
        for page_index in 0..self.config.max_pages {
            let page = self
                .client
                .list_issues_page(
                    token,
                    &self.config.repository,
                    page_cursor.as_deref(),
                    Some(ta_protocol::wire::CODE_HOST_PAGE_MAX_LIMIT),
                    cursor.etag.as_deref().filter(|_| page_index == 0),
                    &cancellation,
                )
                .await
                .map_err(map_code_host_error)?;
            if page.not_modified {
                return Ok(FetchOutcome::NotModified {
                    cursor: SourceCursor {
                        etag: cursor.etag,
                        last_fetched_at_ms: Some(fetched_at_ms),
                    },
                });
            }
            if page_index == 0 {
                etag = page.etag.or(etag);
            }
            all_items.extend(self.items_from_issues(page.issues, fetched_at_ms)?);
            let Some(next_cursor) = page.next_cursor else {
                break;
            };
            page_cursor = Some(next_cursor);
        }
        Ok(FetchOutcome::Items {
            items: all_items,
            cursor: SourceCursor {
                etag,
                last_fetched_at_ms: Some(fetched_at_ms),
            },
        })
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
        let owner = &self.config.repository.owner;
        let name = &self.config.repository.name;
        let key = WorkItemKey::github(owner, name, &external_id)
            .map_err(WorkSourceError::InvalidResponse)?;
        Ok(WorkItem {
            key,
            source: WorkSource::GitHub {
                repo_owner: owner.clone(),
                repo_name: name.clone(),
            },
            external_id,
            title: issue.title,
            body: issue.body.unwrap_or_default(),
            labels,
            url: issue.html_url.unwrap_or_else(|| {
                format!("https://github.com/{owner}/{name}/issues/{}", issue.number)
            }),
            fetched_at_ms,
            status: WorkItemStatus::Available,
            triggered_run_id: None,
        })
    }
}

fn normalize_required(name: &'static str, value: String) -> Result<String, WorkSourceError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(WorkSourceError::InvalidConfig(format!("{name} is empty")));
    }
    Ok(value)
}

fn map_code_host_error(error: CodeHostError) -> WorkSourceError {
    match error {
        CodeHostError::Cancelled => WorkSourceError::Cancelled,
        CodeHostError::InvalidConfig | CodeHostError::InvalidInput => {
            WorkSourceError::InvalidConfig(error.to_string())
        }
        CodeHostError::CredentialsMissing | CodeHostError::CredentialsBackend => {
            WorkSourceError::CredentialsMissing
        }
        CodeHostError::Unauthorized => WorkSourceError::Authentication,
        CodeHostError::Forbidden => WorkSourceError::PermissionDenied,
        CodeHostError::RateLimited { retry_after } => WorkSourceError::RateLimited { retry_after },
        CodeHostError::Unavailable | CodeHostError::OutcomeUnknown => WorkSourceError::Unavailable,
        CodeHostError::NotFound
        | CodeHostError::Conflict
        | CodeHostError::Validation
        | CodeHostError::ResponseTooLarge
        | CodeHostError::InvalidResponse => WorkSourceError::InvalidResponse(error.to_string()),
    }
}

#[cfg(test)]
mod tests;
