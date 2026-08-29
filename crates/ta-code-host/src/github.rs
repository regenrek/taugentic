use std::time::Duration;

use futures_util::StreamExt as _;
use reqwest::{Method, StatusCode, header};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use ta_protocol::wire::{
    CODE_HOST_PAGE_DEFAULT_LIMIT, CODE_HOST_PAGE_MAX_LIMIT, CODE_HOST_RESPONSE_MAX_BYTES,
    CODE_HOST_TEXT_MAX_BYTES, CodeHostCheck, CodeHostCheckStatus, CodeHostComment,
    CodeHostCommentKind, CodeHostPullRequestDetail, CodeHostPullRequestEnsureResult,
    CodeHostPullRequestId, CodeHostPullRequestState, CodeHostPullRequestSummary,
    CodeHostRepositoryRef, CodeHostReview, CodeHostTimelineItem,
};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::{CodeHostAccessToken, CodeHostError};

const GITHUB_API_BASE_URL: &str = "https://api.github.com";
const GITHUB_WEB_ORIGIN: &str = "https://github.com";
const GITHUB_API_VERSION: &str = "2026-03-10";
const GITHUB_ACCEPT: &str = "application/vnd.github+json";
const GITHUB_USER_AGENT: &str = "Taugentic";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubIdentity {
    pub id: u64,
    pub login: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GitHubIssue {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub html_url: Option<String>,
    pub labels: Vec<GitHubIssueLabel>,
    pub pull_request: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GitHubIssueLabel {
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubIssuePage {
    pub issues: Vec<GitHubIssue>,
    pub etag: Option<String>,
    pub not_modified: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestPage {
    pub pull_requests: Vec<CodeHostPullRequestSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubPullRequestActivityPage {
    pub comments: Vec<CodeHostComment>,
    pub reviews: Vec<CodeHostReview>,
    pub timeline: Vec<CodeHostTimelineItem>,
    pub next_cursor: Option<String>,
}

#[derive(Clone)]
pub struct GitHubClient {
    client: reqwest::Client,
    api_base_url: Url,
    web_origin: String,
}

pub fn github_repository_from_remote_url(
    value: &str,
) -> Result<CodeHostRepositoryRef, CodeHostError> {
    let value = value.trim();
    let repository_path = if let Some(path) = value.strip_prefix("git@github.com:") {
        path.to_string()
    } else {
        let url = Url::parse(value).map_err(|_| CodeHostError::InvalidInput)?;
        if !matches!(url.scheme(), "https" | "ssh")
            || !url
                .host_str()
                .is_some_and(|host| host.eq_ignore_ascii_case("github.com"))
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || (url.scheme() == "https" && !url.username().is_empty())
            || (url.scheme() == "ssh" && url.username() != "git")
        {
            return Err(CodeHostError::InvalidInput);
        }
        url.path().trim_start_matches('/').to_string()
    };
    let repository_path = repository_path
        .trim_end_matches('/')
        .trim_end_matches(".git");
    let (owner, name) = repository_path
        .split_once('/')
        .ok_or(CodeHostError::InvalidInput)?;
    if name.contains('/') || !valid_path_part(owner) || !valid_path_part(name) {
        return Err(CodeHostError::InvalidInput);
    }
    Ok(CodeHostRepositoryRef {
        provider: ta_protocol::wire::CodeHostProviderKind::GitHub,
        host: "github.com".to_string(),
        owner: owner.to_string(),
        name: name.to_string(),
    })
}

pub fn github_https_repository_url(
    repository: &CodeHostRepositoryRef,
) -> Result<String, CodeHostError> {
    if repository.provider != ta_protocol::wire::CodeHostProviderKind::GitHub
        || !repository.host.eq_ignore_ascii_case("github.com")
        || !valid_path_part(&repository.owner)
        || !valid_path_part(&repository.name)
    {
        return Err(CodeHostError::InvalidInput);
    }
    Ok(format!(
        "https://github.com/{}/{}.git",
        repository.owner, repository.name
    ))
}

impl GitHubClient {
    pub fn github_dot_com() -> Result<Self, CodeHostError> {
        Self::with_endpoints(GITHUB_API_BASE_URL, GITHUB_WEB_ORIGIN)
    }

    pub fn with_endpoints(api_base_url: &str, web_origin: &str) -> Result<Self, CodeHostError> {
        let api_base_url = validated_base_url(api_base_url)?;
        let web_origin = validated_origin(web_origin)?;
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| CodeHostError::InvalidConfig)?;
        Ok(Self {
            client,
            api_base_url,
            web_origin,
        })
    }

    pub fn web_origin(&self) -> &str {
        &self.web_origin
    }

    pub async fn validate_account(
        &self,
        token: &CodeHostAccessToken,
        cancellation: &CancellationToken,
    ) -> Result<GitHubIdentity, CodeHostError> {
        #[derive(Deserialize)]
        struct Identity {
            id: u64,
            login: String,
        }
        let identity: Identity = self
            .get_json(token, self.url(&["user"])?, cancellation)
            .await?;
        Ok(GitHubIdentity {
            id: identity.id,
            login: bounded_text(identity.login)?,
        })
    }

    pub async fn list_issues_page(
        &self,
        token: &CodeHostAccessToken,
        repository: &CodeHostRepositoryRef,
        cursor: Option<&str>,
        limit: Option<u32>,
        etag: Option<&str>,
        cancellation: &CancellationToken,
    ) -> Result<GitHubIssuePage, CodeHostError> {
        self.require_repository(repository)?;
        let page = decode_page(cursor)?;
        let limit = page_limit(limit);
        let mut url = self.repository_url(repository, &["issues"])?;
        url.query_pairs_mut()
            .append_pair("state", "open")
            .append_pair("per_page", &limit.to_string())
            .append_pair("page", &page.to_string());
        let mut request = self.request(Method::GET, token, url);
        if let Some(etag) = etag {
            request = request.header(header::IF_NONE_MATCH, etag);
        }
        let response = send(request, cancellation, false).await?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(GitHubIssuePage {
                issues: Vec::new(),
                etag: etag.map(str::to_string),
                not_modified: true,
                next_cursor: None,
            });
        }
        let headers = response.headers().clone();
        let response = require_success(response).await?;
        let issues = decode_json::<Vec<GitHubIssue>>(response).await?;
        Ok(GitHubIssuePage {
            issues,
            etag: headers
                .get(header::ETAG)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            not_modified: false,
            next_cursor: next_cursor(&headers),
        })
    }

    pub async fn list_pull_requests(
        &self,
        token: &CodeHostAccessToken,
        repository: &CodeHostRepositoryRef,
        cursor: Option<&str>,
        limit: Option<u32>,
        head: Option<&str>,
        base: Option<&str>,
        cancellation: &CancellationToken,
    ) -> Result<GitHubPullRequestPage, CodeHostError> {
        self.require_repository(repository)?;
        let page = decode_page(cursor)?;
        let limit = page_limit(limit);
        let mut url = self.repository_url(repository, &["pulls"])?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("state", "open")
                .append_pair("per_page", &limit.to_string())
                .append_pair("page", &page.to_string());
            if let Some(head) = head {
                query.append_pair("head", head);
            }
            if let Some(base) = base {
                query.append_pair("base", base);
            }
        }
        let response = self.get_response(token, url, cancellation).await?;
        let headers = response.headers().clone();
        let rows = decode_json::<Vec<GitHubPullRequest>>(response).await?;
        Ok(GitHubPullRequestPage {
            pull_requests: rows
                .into_iter()
                .map(|row| row.into_summary(repository))
                .collect::<Result<_, _>>()?,
            next_cursor: next_cursor(&headers),
        })
    }

    pub async fn pull_request_detail(
        &self,
        token: &CodeHostAccessToken,
        repository: &CodeHostRepositoryRef,
        number: u64,
        cancellation: &CancellationToken,
    ) -> Result<CodeHostPullRequestDetail, CodeHostError> {
        self.require_repository(repository)?;
        let url = self.repository_url(repository, &["pulls", &number.to_string()])?;
        let row: GitHubPullRequest = self.get_json(token, url, cancellation).await?;
        row.into_detail(repository)
    }

    pub async fn ensure_pull_request(
        &self,
        token: &CodeHostAccessToken,
        base_repository: &CodeHostRepositoryRef,
        head_repository: &CodeHostRepositoryRef,
        head_branch: &str,
        base_branch: &str,
        title: &str,
        body: &str,
        draft: bool,
        cancellation: &CancellationToken,
    ) -> Result<CodeHostPullRequestEnsureResult, CodeHostError> {
        self.require_repository(base_repository)?;
        self.require_repository(head_repository)?;
        let head = format!("{}:{head_branch}", head_repository.owner);
        let existing = self
            .list_pull_requests(
                token,
                base_repository,
                None,
                Some(CODE_HOST_PAGE_MAX_LIMIT),
                Some(&head),
                Some(base_branch),
                cancellation,
            )
            .await?;
        match existing.pull_requests.as_slice() {
            [pull_request] => {
                return Ok(CodeHostPullRequestEnsureResult {
                    pull_request: pull_request.clone(),
                    created: false,
                });
            }
            [] => {}
            _ => return Err(CodeHostError::Conflict),
        }
        let url = self.repository_url(base_repository, &["pulls"])?;
        let request = CreatePullRequest {
            title: bounded_text(title.to_string())?,
            body: bounded_text(body.to_string())?,
            head,
            base: bounded_text(base_branch.to_string())?,
            draft,
        };
        let row: GitHubPullRequest = self.post_json(token, url, &request, cancellation).await?;
        Ok(CodeHostPullRequestEnsureResult {
            pull_request: row.into_summary(base_repository)?,
            created: true,
        })
    }

    pub async fn pull_request_checks(
        &self,
        token: &CodeHostAccessToken,
        repository: &CodeHostRepositoryRef,
        head_sha: &str,
        cancellation: &CancellationToken,
    ) -> Result<Vec<CodeHostCheck>, CodeHostError> {
        self.require_repository(repository)?;
        let checks_url = self.repository_url(repository, &["commits", head_sha, "check-runs"])?;
        let status_url = self.repository_url(repository, &["commits", head_sha, "status"])?;
        let (checks, statuses) = futures_util::future::try_join(
            self.get_json::<GitHubCheckRuns>(token, checks_url, cancellation),
            self.get_json::<GitHubCombinedStatus>(token, status_url, cancellation),
        )
        .await?;
        let mut normalized = checks
            .check_runs
            .into_iter()
            .map(GitHubCheckRun::into_check)
            .collect::<Result<Vec<_>, _>>()?;
        normalized.extend(
            statuses
                .statuses
                .into_iter()
                .map(GitHubCommitStatus::into_check)
                .collect::<Result<Vec<_>, _>>()?,
        );
        if normalized.len() > CODE_HOST_PAGE_MAX_LIMIT as usize * 2 {
            return Err(CodeHostError::ResponseTooLarge);
        }
        Ok(normalized)
    }

    pub async fn pull_request_activity(
        &self,
        token: &CodeHostAccessToken,
        repository: &CodeHostRepositoryRef,
        number: u64,
        cursor: Option<&str>,
        limit: Option<u32>,
        cancellation: &CancellationToken,
    ) -> Result<GitHubPullRequestActivityPage, CodeHostError> {
        self.require_repository(repository)?;
        let page = decode_page(cursor)?;
        let limit = page_limit(limit);
        let issue_comments =
            self.repository_url(repository, &["issues", &number.to_string(), "comments"])?;
        let review_comments =
            self.repository_url(repository, &["pulls", &number.to_string(), "comments"])?;
        let reviews =
            self.repository_url(repository, &["pulls", &number.to_string(), "reviews"])?;
        let timeline =
            self.repository_url(repository, &["issues", &number.to_string(), "timeline"])?;
        let add_page = |mut url: Url| {
            url.query_pairs_mut()
                .append_pair("per_page", &limit.to_string())
                .append_pair("page", &page.to_string());
            url
        };
        let issue_future = self.get_response(token, add_page(issue_comments), cancellation);
        let review_comment_future =
            self.get_response(token, add_page(review_comments), cancellation);
        let review_future = self.get_response(token, add_page(reviews), cancellation);
        let timeline_future = self.get_response(token, add_page(timeline), cancellation);
        let (issue_response, review_comment_response, review_response, timeline_response) = futures_util::try_join!(
            issue_future,
            review_comment_future,
            review_future,
            timeline_future
        )?;
        let has_next = [
            &issue_response,
            &review_comment_response,
            &review_response,
            &timeline_response,
        ]
        .iter()
        .any(|response| next_cursor(response.headers()).is_some());
        let mut comments = decode_json::<Vec<GitHubIssueComment>>(issue_response)
            .await?
            .into_iter()
            .map(|comment| comment.into_comment(CodeHostCommentKind::Conversation))
            .collect::<Result<Vec<_>, _>>()?;
        comments.extend(
            decode_json::<Vec<GitHubReviewComment>>(review_comment_response)
                .await?
                .into_iter()
                .map(GitHubReviewComment::into_comment)
                .collect::<Result<Vec<_>, _>>()?,
        );
        let reviews = decode_json::<Vec<GitHubReview>>(review_response)
            .await?
            .into_iter()
            .map(GitHubReview::into_review)
            .collect::<Result<Vec<_>, _>>()?;
        let timeline = decode_json::<Vec<GitHubTimelineEvent>>(timeline_response)
            .await?
            .into_iter()
            .map(GitHubTimelineEvent::into_timeline)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(GitHubPullRequestActivityPage {
            comments,
            reviews,
            timeline,
            next_cursor: has_next.then(|| (page + 1).to_string()),
        })
    }

    pub async fn create_comment(
        &self,
        token: &CodeHostAccessToken,
        repository: &CodeHostRepositoryRef,
        number: u64,
        body: &str,
        cancellation: &CancellationToken,
    ) -> Result<CodeHostComment, CodeHostError> {
        self.require_repository(repository)?;
        let url = self.repository_url(repository, &["issues", &number.to_string(), "comments"])?;
        let row: GitHubIssueComment = self
            .post_json(
                token,
                url,
                &CreateComment {
                    body: bounded_text(body.to_string())?,
                },
                cancellation,
            )
            .await?;
        row.into_comment(CodeHostCommentKind::Conversation)
    }

    fn request(
        &self,
        method: Method,
        token: &CodeHostAccessToken,
        url: Url,
    ) -> reqwest::RequestBuilder {
        self.client
            .request(method, url)
            .bearer_auth(token.expose_secret())
            .header(header::ACCEPT, GITHUB_ACCEPT)
            .header(header::USER_AGENT, GITHUB_USER_AGENT)
            .header("x-github-api-version", GITHUB_API_VERSION)
    }

    async fn get_response(
        &self,
        token: &CodeHostAccessToken,
        url: Url,
        cancellation: &CancellationToken,
    ) -> Result<reqwest::Response, CodeHostError> {
        require_success(send(self.request(Method::GET, token, url), cancellation, false).await?)
            .await
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        token: &CodeHostAccessToken,
        url: Url,
        cancellation: &CancellationToken,
    ) -> Result<T, CodeHostError> {
        decode_json(self.get_response(token, url, cancellation).await?).await
    }

    async fn post_json<T: DeserializeOwned, B: Serialize>(
        &self,
        token: &CodeHostAccessToken,
        url: Url,
        body: &B,
        cancellation: &CancellationToken,
    ) -> Result<T, CodeHostError> {
        let request = self.request(Method::POST, token, url).json(body);
        decode_json(require_success(send(request, cancellation, true).await?).await?).await
    }

    fn url(&self, path: &[&str]) -> Result<Url, CodeHostError> {
        append_path(self.api_base_url.clone(), path)
    }

    fn repository_url(
        &self,
        repository: &CodeHostRepositoryRef,
        path: &[&str],
    ) -> Result<Url, CodeHostError> {
        let mut full = vec!["repos", repository.owner.as_str(), repository.name.as_str()];
        full.extend_from_slice(path);
        self.url(&full)
    }

    fn require_repository(&self, repository: &CodeHostRepositoryRef) -> Result<(), CodeHostError> {
        if repository.host.eq_ignore_ascii_case("github.com")
            && matches!(
                repository.provider,
                ta_protocol::wire::CodeHostProviderKind::GitHub
            )
            && valid_path_part(&repository.owner)
            && valid_path_part(&repository.name)
        {
            return Ok(());
        }
        Err(CodeHostError::InvalidInput)
    }
}

async fn send(
    request: reqwest::RequestBuilder,
    cancellation: &CancellationToken,
    mutation: bool,
) -> Result<reqwest::Response, CodeHostError> {
    tokio::select! {
        _ = cancellation.cancelled() => Err(CodeHostError::Cancelled),
        response = request.send() => response.map_err(|error| {
            if mutation && !error.is_connect() {
                CodeHostError::OutcomeUnknown
            } else {
                CodeHostError::Unavailable
            }
        }),
    }
}

async fn require_success(response: reqwest::Response) -> Result<reqwest::Response, CodeHostError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let retry_after = retry_after(response.headers());
    let error = match status {
        StatusCode::UNAUTHORIZED => CodeHostError::Unauthorized,
        StatusCode::FORBIDDEN => {
            if retry_after.is_some() {
                CodeHostError::RateLimited { retry_after }
            } else {
                CodeHostError::Forbidden
            }
        }
        StatusCode::NOT_FOUND => CodeHostError::NotFound,
        StatusCode::CONFLICT => CodeHostError::Conflict,
        StatusCode::UNPROCESSABLE_ENTITY => CodeHostError::Validation,
        StatusCode::TOO_MANY_REQUESTS => CodeHostError::RateLimited { retry_after },
        status if status.is_server_error() => CodeHostError::Unavailable,
        _ => CodeHostError::Validation,
    };
    Err(error)
}

async fn decode_json<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, CodeHostError> {
    let bytes = bounded_body(response).await?;
    serde_json::from_slice(&bytes).map_err(|_| CodeHostError::InvalidResponse)
}

async fn bounded_body(response: reqwest::Response) -> Result<Vec<u8>, CodeHostError> {
    if response
        .content_length()
        .is_some_and(|length| length > CODE_HOST_RESPONSE_MAX_BYTES as u64)
    {
        return Err(CodeHostError::ResponseTooLarge);
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| CodeHostError::Unavailable)?;
        if body.len().saturating_add(chunk.len()) > CODE_HOST_RESPONSE_MAX_BYTES {
            return Err(CodeHostError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn bounded_text(value: String) -> Result<String, CodeHostError> {
    if value.len() > CODE_HOST_TEXT_MAX_BYTES {
        return Err(CodeHostError::ResponseTooLarge);
    }
    Ok(value)
}

fn page_limit(limit: Option<u32>) -> u32 {
    limit
        .unwrap_or(CODE_HOST_PAGE_DEFAULT_LIMIT)
        .clamp(1, CODE_HOST_PAGE_MAX_LIMIT)
}

fn decode_page(cursor: Option<&str>) -> Result<u32, CodeHostError> {
    match cursor {
        None => Ok(1),
        Some(cursor) => cursor
            .parse::<u32>()
            .ok()
            .filter(|page| *page > 0)
            .ok_or(CodeHostError::InvalidInput),
    }
}

fn next_cursor(headers: &header::HeaderMap) -> Option<String> {
    let link = headers.get(header::LINK)?.to_str().ok()?;
    link.split(',').find_map(|part| {
        let (url, relation) = part.trim().split_once(';')?;
        if !relation.contains("rel=\"next\"") {
            return None;
        }
        let url = Url::parse(url.trim().trim_start_matches('<').trim_end_matches('>')).ok()?;
        url.query_pairs()
            .find(|(key, _)| key == "page")
            .map(|(_, value)| value.into_owned())
    })
}

fn retry_after(headers: &header::HeaderMap) -> Option<Duration> {
    headers
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|seconds| Duration::from_secs(seconds.max(1)))
}

fn validated_base_url(value: &str) -> Result<Url, CodeHostError> {
    let mut url = Url::parse(value).map_err(|_| CodeHostError::InvalidConfig)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(CodeHostError::InvalidConfig);
    }
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Ok(url)
}

fn validated_origin(value: &str) -> Result<String, CodeHostError> {
    let url = validated_base_url(value)?;
    let host = url.host_str().ok_or(CodeHostError::InvalidConfig)?;
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    Ok(format!("{}://{host}{port}", url.scheme()))
}

fn append_path(mut url: Url, path: &[&str]) -> Result<Url, CodeHostError> {
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| CodeHostError::InvalidConfig)?;
    segments.pop_if_empty();
    for part in path {
        if !valid_path_part(part) {
            return Err(CodeHostError::InvalidInput);
        }
        segments.push(part);
    }
    drop(segments);
    Ok(url)
}

fn valid_path_part(value: &str) -> bool {
    !value.trim().is_empty() && !value.contains(['/', '\\', '\0'])
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRepository {
    name: String,
    owner: GitHubUser,
}

#[derive(Debug, Deserialize)]
struct GitHubBranchRef {
    #[serde(rename = "ref")]
    branch: String,
    sha: String,
    repo: GitHubRepository,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequest {
    id: u64,
    number: u64,
    title: String,
    body: Option<String>,
    state: String,
    draft: Option<bool>,
    user: GitHubUser,
    head: GitHubBranchRef,
    base: GitHubBranchRef,
    html_url: String,
    updated_at: String,
    merged_at: Option<String>,
    mergeable: Option<bool>,
    additions: Option<u32>,
    deletions: Option<u32>,
    changed_files: Option<u32>,
}

impl GitHubPullRequest {
    fn into_summary(
        self,
        base_repository: &CodeHostRepositoryRef,
    ) -> Result<CodeHostPullRequestSummary, CodeHostError> {
        let state = if self.merged_at.is_some() {
            CodeHostPullRequestState::Merged
        } else if self.state == "open" {
            CodeHostPullRequestState::Open
        } else {
            CodeHostPullRequestState::Closed
        };
        Ok(CodeHostPullRequestSummary {
            id: CodeHostPullRequestId::new(self.id.to_string())
                .map_err(|_| CodeHostError::InvalidResponse)?,
            number: self.number,
            title: bounded_text(self.title)?,
            state,
            draft: self.draft.unwrap_or(false),
            author_login: bounded_text(self.user.login)?,
            head_repository: CodeHostRepositoryRef {
                provider: base_repository.provider,
                host: base_repository.host.clone(),
                owner: bounded_text(self.head.repo.owner.login)?,
                name: bounded_text(self.head.repo.name)?,
            },
            head_branch: bounded_text(self.head.branch)?,
            head_sha: bounded_text(self.head.sha)?,
            base_repository: CodeHostRepositoryRef {
                provider: base_repository.provider,
                host: base_repository.host.clone(),
                owner: bounded_text(self.base.repo.owner.login)?,
                name: bounded_text(self.base.repo.name)?,
            },
            base_branch: bounded_text(self.base.branch)?,
            web_url: bounded_text(self.html_url)?,
            updated_at: bounded_text(self.updated_at)?,
        })
    }

    fn into_detail(
        self,
        base_repository: &CodeHostRepositoryRef,
    ) -> Result<CodeHostPullRequestDetail, CodeHostError> {
        let body = bounded_text(self.body.clone().unwrap_or_default())?;
        let mergeable = self.mergeable;
        let additions = self.additions.unwrap_or_default();
        let deletions = self.deletions.unwrap_or_default();
        let changed_files = self.changed_files.unwrap_or_default();
        Ok(CodeHostPullRequestDetail {
            summary: self.into_summary(base_repository)?,
            body,
            mergeable,
            additions,
            deletions,
            changed_files,
        })
    }
}

#[derive(Serialize)]
struct CreatePullRequest {
    title: String,
    body: String,
    head: String,
    base: String,
    draft: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubCheckRuns {
    check_runs: Vec<GitHubCheckRun>,
}

#[derive(Debug, Deserialize)]
struct GitHubCheckRun {
    id: u64,
    name: String,
    status: String,
    conclusion: Option<String>,
    details_url: Option<String>,
}

impl GitHubCheckRun {
    fn into_check(self) -> Result<CodeHostCheck, CodeHostError> {
        Ok(CodeHostCheck {
            id: format!("check-run:{}", self.id),
            name: bounded_text(self.name)?,
            status: check_status(&self.status),
            conclusion: self.conclusion.map(bounded_text).transpose()?,
            details_url: self.details_url.map(bounded_text).transpose()?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct GitHubCombinedStatus {
    statuses: Vec<GitHubCommitStatus>,
}

#[derive(Debug, Deserialize)]
struct GitHubCommitStatus {
    id: u64,
    context: String,
    state: String,
    target_url: Option<String>,
    description: Option<String>,
}

impl GitHubCommitStatus {
    fn into_check(self) -> Result<CodeHostCheck, CodeHostError> {
        let conclusion = match self.state.as_str() {
            "success" => Some("success".to_string()),
            "failure" | "error" => Some("failure".to_string()),
            _ => self.description.map(bounded_text).transpose()?,
        };
        Ok(CodeHostCheck {
            id: format!("status:{}", self.id),
            name: bounded_text(self.context)?,
            status: if self.state == "pending" {
                CodeHostCheckStatus::InProgress
            } else {
                CodeHostCheckStatus::Completed
            },
            conclusion,
            details_url: self.target_url.map(bounded_text).transpose()?,
        })
    }
}

fn check_status(value: &str) -> CodeHostCheckStatus {
    match value {
        "queued" => CodeHostCheckStatus::Queued,
        "in_progress" => CodeHostCheckStatus::InProgress,
        "completed" => CodeHostCheckStatus::Completed,
        _ => CodeHostCheckStatus::Unknown,
    }
}

#[derive(Debug, Deserialize)]
struct GitHubIssueComment {
    id: u64,
    user: GitHubUser,
    body: String,
    html_url: String,
    created_at: String,
}

impl GitHubIssueComment {
    fn into_comment(self, kind: CodeHostCommentKind) -> Result<CodeHostComment, CodeHostError> {
        Ok(CodeHostComment {
            id: self.id.to_string(),
            kind,
            author_login: bounded_text(self.user.login)?,
            body: bounded_text(self.body)?,
            web_url: bounded_text(self.html_url)?,
            created_at: bounded_text(self.created_at)?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct GitHubReviewComment {
    id: u64,
    user: GitHubUser,
    body: String,
    html_url: String,
    created_at: String,
}

impl GitHubReviewComment {
    fn into_comment(self) -> Result<CodeHostComment, CodeHostError> {
        GitHubIssueComment {
            id: self.id,
            user: self.user,
            body: self.body,
            html_url: self.html_url,
            created_at: self.created_at,
        }
        .into_comment(CodeHostCommentKind::Review)
    }
}

#[derive(Debug, Deserialize)]
struct GitHubReview {
    id: u64,
    user: GitHubUser,
    state: String,
    body: Option<String>,
    html_url: String,
    submitted_at: Option<String>,
}

impl GitHubReview {
    fn into_review(self) -> Result<CodeHostReview, CodeHostError> {
        Ok(CodeHostReview {
            id: self.id.to_string(),
            author_login: bounded_text(self.user.login)?,
            state: bounded_text(self.state)?,
            body: bounded_text(self.body.unwrap_or_default())?,
            web_url: bounded_text(self.html_url)?,
            submitted_at: bounded_text(self.submitted_at.unwrap_or_default())?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct GitHubTimelineEvent {
    id: Option<u64>,
    event: Option<String>,
    actor: Option<GitHubUser>,
    created_at: Option<String>,
    body: Option<String>,
}

impl GitHubTimelineEvent {
    fn into_timeline(self) -> Result<CodeHostTimelineItem, CodeHostError> {
        let kind = bounded_text(self.event.unwrap_or_else(|| "unknown".to_string()))?;
        Ok(CodeHostTimelineItem {
            id: self
                .id
                .map(|id| id.to_string())
                .unwrap_or_else(|| format!("{kind}:unknown")),
            kind: kind.clone(),
            actor_login: bounded_text(self.actor.map(|actor| actor.login).unwrap_or_default())?,
            summary: bounded_text(self.body.unwrap_or(kind))?,
            created_at: bounded_text(self.created_at.unwrap_or_default())?,
        })
    }
}

#[derive(Serialize)]
struct CreateComment {
    body: String,
}

#[cfg(test)]
mod tests;
