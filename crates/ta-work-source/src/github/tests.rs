use std::error::Error;

use serde_json::json;
use ta_code_host::{CodeHostAccessToken, GitHubClient};
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::FetchOutcome;
use ta_protocol::wire::{SourceCursor, WorkSourceLabelFilter};

#[tokio::test]
async fn maps_issues_and_filters_pull_requests_without_owning_http_policy()
-> Result<(), Box<dyn Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/regenrek/taugentic/issues"))
        .and(query_param("page", "1"))
        .and(header("x-github-api-version", "2026-03-10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            issue_json(1, "First", ["ready"], false),
            issue_json(2, "PR", ["ready"], true)
        ])))
        .expect(1)
        .mount(&server)
        .await;

    let outcome = provider(&server)?
        .fetch(
            &CodeHostAccessToken::new("test-token")?,
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
    Ok(())
}

#[tokio::test]
async fn preserves_etag_on_not_modified() -> Result<(), Box<dyn Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/regenrek/taugentic/issues"))
        .and(header("if-none-match", "\"etag-1\""))
        .respond_with(ResponseTemplate::new(304))
        .expect(1)
        .mount(&server)
        .await;

    let outcome = provider(&server)?
        .fetch(
            &CodeHostAccessToken::new("test-token")?,
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
async fn maps_rate_limit_without_retrying() -> Result<(), Box<dyn Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/regenrek/taugentic/issues"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "7"))
        .expect(1)
        .mount(&server)
        .await;

    let error = provider(&server)?
        .fetch(
            &CodeHostAccessToken::new("test-token")?,
            SourceCursor::empty(),
            100,
            CancellationToken::new(),
        )
        .await
        .err()
        .ok_or("expected error")?;
    let WorkSourceError::RateLimited { retry_after } = error else {
        return Err("expected rate limit".into());
    };
    assert_eq!(retry_after.map(|duration| duration.as_secs()), Some(7));
    Ok(())
}

fn provider(server: &MockServer) -> Result<GitHubIssueProvider, Box<dyn Error>> {
    let client = GitHubClient::with_endpoints(&server.uri(), "https://github.com")?;
    let config = GitHubProviderConfig::github_dot_com(
        "regenrek",
        "taugentic",
        WorkSourceLabelFilter::AnyOf(vec!["ready".to_string()]),
    )?
    .with_max_pages(1)?;
    Ok(GitHubIssueProvider::new(client, config))
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
