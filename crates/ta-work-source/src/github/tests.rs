use std::error::Error;
use std::sync::Mutex;

use reqwest::StatusCode;
use serde_json::json;
use ta_host_platform::{HostSecretError, HostSecretKey, HostSecretStore, HostSecretValue};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::{FetchOutcome, SourceCursor, WorkSourceLabelFilter};

#[tokio::test]
async fn fetches_paginated_issues_and_filters_pull_requests() -> Result<(), Box<dyn Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/regenrek/taugentic/issues"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            issue_json(1, "First", ["ready"], false),
            issue_json(2, "PR", ["ready"], true)
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let provider = provider(&server)?.with_max_pages(1)?;
    let outcome = GitHubIssueProvider::new(provider)
        .fetch(
            &token()?,
            SourceCursor::empty(),
            100,
            CancellationToken::new(),
        )
        .await?;

    let FetchOutcome::Items { items, .. } = outcome else {
        return Err("expected items".into());
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key.as_str(), "github:regenrek/taugentic#1");
    assert_eq!(items[0].title, "First");
    Ok(())
}

#[tokio::test]
async fn sends_if_none_match_and_preserves_cursor_on_304() -> Result<(), Box<dyn Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/regenrek/taugentic/issues"))
        .and(header("if-none-match", "\"etag-1\""))
        .respond_with(ResponseTemplate::new(304))
        .expect(1)
        .mount(&server)
        .await;

    let outcome = GitHubIssueProvider::new(provider(&server)?)
        .fetch(
            &token()?,
            SourceCursor {
                etag: Some("\"etag-1\"".to_string()),
                last_fetched_at_ms: Some(50),
            },
            120,
            CancellationToken::new(),
        )
        .await?;

    let FetchOutcome::NotModified { cursor } = outcome else {
        return Err("expected not modified".into());
    };
    assert_eq!(cursor.etag.as_deref(), Some("\"etag-1\""));
    assert_eq!(cursor.last_fetched_at_ms, Some(120));
    Ok(())
}

#[tokio::test]
async fn captures_etag_from_first_page() -> Result<(), Box<dyn Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/regenrek/taugentic/issues"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "\"etag-next\"")
                .set_body_json(json!([issue_json(3, "Third", ["ready"], false)])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let outcome = GitHubIssueProvider::new(provider(&server)?.with_max_pages(1)?)
        .fetch(
            &token()?,
            SourceCursor::empty(),
            100,
            CancellationToken::new(),
        )
        .await?;

    let FetchOutcome::Items { cursor, .. } = outcome else {
        return Err("expected items".into());
    };
    assert_eq!(cursor.etag.as_deref(), Some("\"etag-next\""));
    Ok(())
}

#[tokio::test]
async fn rate_limit_error_uses_retry_after() -> Result<(), Box<dyn Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/regenrek/taugentic/issues"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "7")
                .set_body_string("rate limited"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let error = GitHubIssueProvider::new(provider(&server)?.with_max_pages(1)?)
        .fetch(
            &token()?,
            SourceCursor::empty(),
            100,
            CancellationToken::new(),
        )
        .await
        .err()
        .ok_or("expected error")?;

    let WorkSourceError::HttpStatus { status, backoff } = error else {
        return Err("expected http status".into());
    };
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(backoff.map(|value| value.retry_after.as_secs()), Some(7));
    Ok(())
}

#[test]
fn host_secret_provider_loads_github_pat_from_store() -> Result<(), Box<dyn Error>> {
    let store = std::sync::Arc::new(MockHostSecretStore::default());
    store.store_secret(
        HostSecretKey::WORK_SOURCE_GITHUB_PAT,
        &HostSecretValue::new("ghp_ssot")?,
    )?;
    let provider = HostSecretsGitHubCredentialProvider::new(store);

    assert_eq!(provider.token()?.as_str(), "ghp_ssot");
    Ok(())
}

#[test]
fn host_secret_provider_reports_missing_github_pat() {
    let store = std::sync::Arc::new(MockHostSecretStore::default());
    let provider = HostSecretsGitHubCredentialProvider::new(store);

    assert!(matches!(
        provider.token(),
        Err(WorkSourceError::CredentialsMissing)
    ));
}

fn provider(server: &MockServer) -> Result<GitHubProviderConfig, WorkSourceError> {
    GitHubProviderConfig::new(
        "regenrek",
        "taugentic",
        WorkSourceLabelFilter::AnyOf(vec!["ready".to_string()]),
    )
    .and_then(|config| config.with_base_url(server.uri()))
}

fn token() -> Result<GitHubToken, WorkSourceError> {
    GitHubToken::new("ghp_test")
}

fn issue_json(
    number: u64,
    title: &str,
    labels: impl IntoIterator<Item = &'static str>,
    pull_request: bool,
) -> serde_json::Value {
    let mut value = json!({
        "number": number,
        "title": title,
        "body": "body",
        "html_url": format!("https://github.com/regenrek/taugentic/issues/{number}"),
        "labels": labels.into_iter().map(|name| json!({ "name": name })).collect::<Vec<_>>()
    });
    if pull_request {
        value["pull_request"] = json!({});
    }
    value
}

#[derive(Default)]
struct MockHostSecretStore {
    value: Mutex<Option<HostSecretValue>>,
}

impl HostSecretStore for MockHostSecretStore {
    fn store_secret(
        &self,
        key: HostSecretKey,
        value: &HostSecretValue,
    ) -> Result<(), HostSecretError> {
        assert_eq!(key, HostSecretKey::WORK_SOURCE_GITHUB_PAT);
        *self.value.lock().map_err(|_| poison_error())? = Some(value.clone());
        Ok(())
    }

    fn load_secret(&self, key: HostSecretKey) -> Result<Option<HostSecretValue>, HostSecretError> {
        assert_eq!(key, HostSecretKey::WORK_SOURCE_GITHUB_PAT);
        self.value
            .lock()
            .map_err(|_| poison_error())
            .map(|value| value.clone())
    }

    fn delete_secret(&self, key: HostSecretKey) -> Result<(), HostSecretError> {
        assert_eq!(key, HostSecretKey::WORK_SOURCE_GITHUB_PAT);
        *self.value.lock().map_err(|_| poison_error())? = None;
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "mock-keyring"
    }
}

fn poison_error() -> HostSecretError {
    HostSecretError::IoError {
        operation: "mock-keyring-lock",
        reason: "lock poisoned".to_string(),
    }
}
