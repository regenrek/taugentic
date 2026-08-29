use std::time::Duration;

use serde_json::{Value, json};
use ta_protocol::wire::{
    CODE_HOST_RESPONSE_MAX_BYTES, CodeHostProviderKind, CodeHostRepositoryRef,
};
use tokio_util::sync::CancellationToken;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use super::*;
use crate::{CodeHostAccessToken, CodeHostError, GitHttpsAuthorization};

fn token() -> CodeHostAccessToken {
    CodeHostAccessToken::new("test-token-value").expect("test token should be valid")
}

fn repository(owner: &str, name: &str) -> CodeHostRepositoryRef {
    CodeHostRepositoryRef {
        provider: CodeHostProviderKind::GitHub,
        host: "github.com".to_string(),
        owner: owner.to_string(),
        name: name.to_string(),
    }
}

fn pull_request(number: u64, head_owner: &str, head_name: &str, head_branch: &str) -> Value {
    json!({
        "id": number + 1000,
        "number": number,
        "title": format!("Change {number}"),
        "body": "A bounded change",
        "state": "open",
        "draft": false,
        "user": { "login": "review-author" },
        "head": {
            "ref": head_branch,
            "sha": "1111111111111111111111111111111111111111",
            "repo": { "name": head_name, "owner": { "login": head_owner } }
        },
        "base": {
            "ref": "main",
            "sha": "2222222222222222222222222222222222222222",
            "repo": { "name": "project", "owner": { "login": "base-owner" } }
        },
        "html_url": format!("https://github.com/base-owner/project/pull/{number}"),
        "updated_at": "2026-08-27T10:00:00Z",
        "merged_at": null,
        "mergeable": true,
        "additions": 3,
        "deletions": 1,
        "changed_files": 2
    })
}

async fn client(server: &MockServer) -> GitHubClient {
    GitHubClient::with_endpoints(&server.uri(), &server.uri())
        .expect("test endpoints should be valid")
}

#[test]
fn remote_parser_accepts_canonical_https_ssh_and_scp_forms() {
    for remote in [
        "https://github.com/example-owner/example-project.git",
        "ssh://git@github.com/example-owner/example-project.git",
        "git@github.com:example-owner/example-project.git",
    ] {
        assert_eq!(
            github_repository_from_remote_url(remote).expect("remote should parse"),
            repository("example-owner", "example-project")
        );
    }
}

#[test]
fn repository_https_url_is_canonical_and_contains_no_credentials() {
    assert_eq!(
        github_https_repository_url(&repository("example-owner", "example-project"))
            .expect("repository should render"),
        "https://github.com/example-owner/example-project.git"
    );
}

#[test]
fn remote_parser_rejects_credentials_queries_and_non_github_hosts() {
    for remote in [
        "https://token@github.com/example-owner/example-project.git",
        "https://github.com/example-owner/example-project.git?token=value",
        "https://example.invalid/example-owner/example-project.git",
        "git@github.com:example-owner/nested/example-project.git",
    ] {
        assert!(matches!(
            github_repository_from_remote_url(remote),
            Err(CodeHostError::InvalidInput)
        ));
    }
}

#[test]
fn credentials_and_git_authorization_debug_output_are_redacted() {
    let token = token();
    let authorization = GitHttpsAuthorization::github("https://github.com", "example-user", &token)
        .expect("authorization should be valid");
    assert_eq!(format!("{token:?}"), "CodeHostAccessToken([REDACTED])");
    let rendered = format!("{authorization:?}");
    assert!(rendered.contains("[REDACTED]"));
    assert!(!rendered.contains(token.expose_secret()));
    assert!(!rendered.contains(authorization.expose_extra_header()));
    assert_eq!(
        authorization.expose_extra_header(),
        "AUTHORIZATION: basic ZXhhbXBsZS11c2VyOnRlc3QtdG9rZW4tdmFsdWU="
    );
    for invalid_login in ["", "two users", "user:name", "user@example"] {
        assert!(matches!(
            GitHttpsAuthorization::github("https://github.com", invalid_login, &token),
            Err(CodeHostError::InvalidInput)
        ));
    }
}

#[tokio::test]
async fn list_request_is_versioned_authenticated_bounded_and_paged_once() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/base-owner/project/pulls"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header(
                    "link",
                    format!(
                        "<{}repos/base-owner/project/pulls?page=3>; rel=\"next\"",
                        server.uri() + "/"
                    ),
                )
                .set_body_json(vec![pull_request(7, "fork-owner", "project", "feature")]),
        )
        .expect(1)
        .mount(&server)
        .await;

    let page = client(&server)
        .await
        .list_pull_requests(
            &token(),
            &repository("base-owner", "project"),
            Some("2"),
            Some(25),
            None,
            None,
            &CancellationToken::new(),
        )
        .await
        .expect("page should load");
    assert_eq!(page.next_cursor.as_deref(), Some("3"));
    assert_eq!(page.pull_requests[0].head_repository.owner, "fork-owner");

    let requests = server
        .received_requests()
        .await
        .expect("requests should be available");
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(
        request
            .url
            .query_pairs()
            .find(|(key, _)| key == "page")
            .map(|(_, value)| value.into_owned())
            .as_deref(),
        Some("2")
    );
    assert_eq!(
        request
            .url
            .query_pairs()
            .find(|(key, _)| key == "per_page")
            .map(|(_, value)| value.into_owned())
            .as_deref(),
        Some("25")
    );
    assert_eq!(
        request
            .headers
            .get("x-github-api-version")
            .and_then(|value| value.to_str().ok()),
        Some("2026-03-10")
    );
    assert_eq!(
        request
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer test-token-value")
    );
    server.verify().await;
}

#[tokio::test]
async fn unavailable_request_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    let error = client(&server)
        .await
        .validate_account(&token(), &CancellationToken::new())
        .await
        .expect_err("unavailable response should fail");
    assert!(matches!(error, CodeHostError::Unavailable));
    assert_eq!(
        server
            .received_requests()
            .await
            .expect("requests should load")
            .len(),
        1
    );
    server.verify().await;
}

#[tokio::test]
async fn oversized_body_is_rejected_before_deserialization() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("x".repeat(CODE_HOST_RESPONSE_MAX_BYTES + 1)),
        )
        .mount(&server)
        .await;
    let error = client(&server)
        .await
        .validate_account(&token(), &CancellationToken::new())
        .await
        .expect_err("oversized body should fail");
    assert!(matches!(error, CodeHostError::ResponseTooLarge));
}

#[tokio::test]
async fn rate_limit_preserves_retry_after_without_sleeping() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "17"))
        .expect(1)
        .mount(&server)
        .await;
    let error = client(&server)
        .await
        .validate_account(&token(), &CancellationToken::new())
        .await
        .expect_err("rate limit should fail");
    assert!(matches!(
        error,
        CodeHostError::RateLimited { retry_after: Some(duration) }
            if duration == Duration::from_secs(17)
    ));
    server.verify().await;
}

#[tokio::test]
async fn permission_denial_is_typed_and_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;
    let error = client(&server)
        .await
        .validate_account(&token(), &CancellationToken::new())
        .await
        .expect_err("permission denial should fail");
    assert!(matches!(error, CodeHostError::Forbidden));
    assert_eq!(
        server
            .received_requests()
            .await
            .expect("requests should load")
            .len(),
        1
    );
    server.verify().await;
}

#[tokio::test]
async fn ensure_open_uses_exact_fork_head_and_base_natural_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/base-owner/project/pulls"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![pull_request(
            9,
            "fork-owner",
            "project",
            "feature",
        )]))
        .expect(1)
        .mount(&server)
        .await;

    let result = client(&server)
        .await
        .ensure_pull_request(
            &token(),
            &repository("base-owner", "project"),
            &repository("fork-owner", "project"),
            "feature",
            "main",
            "Ignored for an existing pull request",
            "",
            false,
            &CancellationToken::new(),
        )
        .await
        .expect("exact existing pull request should be reused");
    assert!(!result.created);
    assert_eq!(result.pull_request.number, 9);
    let requests = server
        .received_requests()
        .await
        .expect("requests should load");
    assert_eq!(requests.len(), 1);
    let query = requests[0].url.query_pairs().collect::<Vec<_>>();
    assert!(
        query
            .iter()
            .any(|(key, value)| key == "head" && value == "fork-owner:feature")
    );
    assert!(
        query
            .iter()
            .any(|(key, value)| key == "base" && value == "main")
    );
    server.verify().await;
}

#[tokio::test]
async fn ensure_open_uses_exact_same_repository_head_and_base_natural_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/base-owner/project/pulls"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![pull_request(
            11,
            "base-owner",
            "project",
            "feature",
        )]))
        .expect(1)
        .mount(&server)
        .await;

    let repository = repository("base-owner", "project");
    let result = client(&server)
        .await
        .ensure_pull_request(
            &token(),
            &repository,
            &repository,
            "feature",
            "main",
            "Ignored for an existing pull request",
            "",
            false,
            &CancellationToken::new(),
        )
        .await
        .expect("exact existing pull request should be reused");
    assert!(!result.created);
    assert_eq!(result.pull_request.number, 11);
    let requests = server
        .received_requests()
        .await
        .expect("requests should load");
    let query = requests[0].url.query_pairs().collect::<Vec<_>>();
    assert!(
        query
            .iter()
            .any(|(key, value)| key == "head" && value == "base-owner:feature")
    );
    assert!(
        query
            .iter()
            .any(|(key, value)| key == "base" && value == "main")
    );
    server.verify().await;
}

#[tokio::test]
async fn ensure_open_rejects_ambiguous_matches_without_mutating() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/base-owner/project/pulls"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vec![
            pull_request(9, "fork-owner", "project", "feature"),
            pull_request(10, "fork-owner", "project", "feature"),
        ]))
        .expect(1)
        .mount(&server)
        .await;
    let error = client(&server)
        .await
        .ensure_pull_request(
            &token(),
            &repository("base-owner", "project"),
            &repository("fork-owner", "project"),
            "feature",
            "main",
            "Change",
            "",
            false,
            &CancellationToken::new(),
        )
        .await
        .expect_err("ambiguous matches must fail");
    assert!(matches!(error, CodeHostError::Conflict));
    assert_eq!(
        server
            .received_requests()
            .await
            .expect("requests should load")
            .len(),
        1
    );
    server.verify().await;
}
