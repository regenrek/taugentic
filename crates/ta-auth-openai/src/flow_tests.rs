use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use serde_json::json;

use crate::client::{FormField, OAuthHttpClient, OAuthHttpFuture, OAuthHttpResponse};
use crate::config::{OAuthConfig, OPENAI_CHATGPT_CLIENT_ID, OPENAI_CHATGPT_ORIGINATOR};
use crate::error::OAuthError;
use crate::flow::{OAuthCode, OpenAiOAuthFlow, TokenSet};

#[tokio::test]
async fn start_builds_authorize_url_and_awaits_callback() -> Result<(), Box<dyn std::error::Error>>
{
    let config = test_config()?;
    let (authorize_url, completion) = OpenAiOAuthFlow::start(config).await?;
    let redirect_uri = authorize_url
        .query_pairs()
        .find(|(key, _value)| key == "redirect_uri")
        .map(|(_key, value)| value.into_owned())
        .ok_or("missing redirect_uri")?;
    let state = authorize_url
        .query_pairs()
        .find(|(key, _value)| key == "state")
        .map(|(_key, value)| value.into_owned())
        .ok_or("missing state")?;
    let port = redirect_uri
        .rsplit_once(':')
        .and_then(|(_prefix, tail)| tail.split('/').next())
        .ok_or("missing port")?
        .parse::<u16>()?;
    let code_task = tokio::spawn(async move { completion.await_code().await });

    send_callback(
        port,
        &format!("/auth/callback?code=auth-code&state={state}"),
    )
    .await?;
    let code = code_task.await??;

    assert_eq!(code.code, "auth-code");
    assert_eq!(code.redirect_uri, redirect_uri);
    assert!(
        authorize_url
            .query_pairs()
            .any(|(key, value)| key == "code_challenge_method" && value == "S256")
    );
    assert!(
        authorize_url
            .query_pairs()
            .any(|(key, value)| key == "id_token_add_organizations" && value == "true")
    );
    assert!(
        authorize_url
            .query_pairs()
            .any(|(key, value)| key == "codex_cli_simplified_flow" && value == "true")
    );
    assert!(
        authorize_url
            .query_pairs()
            .any(|(key, value)| key == "originator" && value == "taugentic-test")
    );
    Ok(())
}

#[tokio::test]
async fn start_pins_codex_client_id_to_codex_originator() -> Result<(), Box<dyn std::error::Error>>
{
    let mut config = test_config()?;
    config.client_id = OPENAI_CHATGPT_CLIENT_ID.to_string();
    config.originator = Some("taugentic".to_string());

    let (authorize_url, _completion) = OpenAiOAuthFlow::start(config).await?;

    assert!(
        authorize_url
            .query_pairs()
            .any(|(key, value)| key == "originator" && value == OPENAI_CHATGPT_ORIGINATOR)
    );
    assert!(
        !authorize_url
            .query_pairs()
            .any(|(key, value)| key == "originator" && value == "taugentic")
    );
    Ok(())
}

#[tokio::test]
async fn exchange_code_succeeds_when_id_token_has_organization_id()
-> Result<(), Box<dyn std::error::Error>> {
    let config = test_config()?;
    let id_token = test_id_token()?;
    let http = MockHttpClient::new(vec![
        OAuthHttpResponse {
            status: 200,
            body: json!({
                "access_token": "oauth-access",
                "refresh_token": "refresh",
                "id_token": id_token,
                "expires_in": 3600,
                "scope": "openid profile"
            })
            .to_string(),
        },
        OAuthHttpResponse {
            status: 200,
            body: json!({ "access_token": "api-access" }).to_string(),
        },
    ]);
    let code = OAuthCode::new(
        "auth-code",
        "verifier",
        "http://localhost:1455/auth/callback",
        &config,
    );

    let tokens = OpenAiOAuthFlow::exchange_code(&http, code).await?;

    assert_eq!(
        tokens,
        TokenSet {
            access_token: "oauth-access".to_string(),
            refresh_token: "refresh".to_string(),
            id_token: Some(test_id_token()?),
            expires_in: Some(3600),
            scope: Some("openid profile".to_string()),
            api_access_token: Some("api-access".to_string()),
            account_info: Some(crate::token::claims::ChatGptAccountInfo {
                email: Some("user@example.com".to_string()),
                account_id: Some("acc_123".to_string()),
                organization_id: Some("org_123".to_string()),
                user_id: None,
                plan_type: Some("plus".to_string()),
                is_fedramp: Some(false),
                expires_at: Some(1_800_000_000),
            }),
        }
    );
    let requests = http.requests()?;
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0]
            .iter()
            .any(|field| { field.name == "grant_type" && field.value == "authorization_code" })
    );
    assert!(
        requests[1]
            .iter()
            .any(|field| { field.name == "requested_token" && field.value == "openai-api-key" })
    );
    Ok(())
}

#[tokio::test]
async fn exchange_code_succeeds_when_id_token_missing_organization_id()
-> Result<(), Box<dyn std::error::Error>> {
    let config = test_config()?;
    let id_token = test_id_token_without_organization()?;
    let http = MockHttpClient::new(vec![
        OAuthHttpResponse {
            status: 200,
            body: json!({
                "access_token": "oauth-access",
                "refresh_token": "refresh",
                "id_token": id_token,
                "expires_in": 3600,
                "scope": "openid profile"
            })
            .to_string(),
        },
        OAuthHttpResponse {
            status: 401,
            body: json!({
                "error": "invalid_request",
                "error_description": "Invalid ID token: missing organization_id"
            })
            .to_string(),
        },
    ]);
    let code = OAuthCode::new(
        "auth-code",
        "verifier",
        "http://localhost:1455/auth/callback",
        &config,
    );

    let tokens = OpenAiOAuthFlow::exchange_code(&http, code).await?;

    assert_eq!(tokens.access_token, "oauth-access");
    assert_eq!(tokens.api_access_token, None);
    assert_eq!(
        tokens
            .account_info
            .as_ref()
            .and_then(|info| info.organization_id.as_deref()),
        None
    );
    assert_eq!(http.requests()?.len(), 2);
    Ok(())
}

#[tokio::test]
async fn exchange_code_bubbles_non_org_token_exchange_4xx() -> Result<(), Box<dyn std::error::Error>>
{
    let config = test_config()?;
    let http = MockHttpClient::new(vec![
        OAuthHttpResponse {
            status: 200,
            body: json!({
                "access_token": "oauth-access",
                "refresh_token": "refresh",
                "id_token": test_id_token()?,
                "expires_in": 3600,
                "scope": "openid profile"
            })
            .to_string(),
        },
        OAuthHttpResponse {
            status: 400,
            body: json!({
                "error": "invalid_request",
                "error_description": "Invalid ID token: wrong audience"
            })
            .to_string(),
        },
    ]);
    let code = OAuthCode::new(
        "auth-code",
        "verifier",
        "http://localhost:1455/auth/callback",
        &config,
    );

    let result = OpenAiOAuthFlow::exchange_code(&http, code).await;

    assert!(matches!(
        result,
        Err(OAuthError::TokenEndpointStatus {
            status: 400,
            error_code: Some(_),
            ..
        })
    ));
    assert_eq!(http.requests()?.len(), 2);
    Ok(())
}

#[tokio::test]
async fn exchange_code_returns_status_error_without_token_values()
-> Result<(), Box<dyn std::error::Error>> {
    let config = test_config()?;
    let http = MockHttpClient::new(vec![OAuthHttpResponse {
        status: 400,
        body: json!({
            "error": "invalid_grant",
            "error_description": "authorization code was rejected"
        })
        .to_string(),
    }]);
    let code = OAuthCode::new(
        "secret-code",
        "secret-verifier",
        "http://localhost:1455/auth/callback",
        &config,
    );

    let result = OpenAiOAuthFlow::exchange_code(&http, code).await;

    let message = match &result {
        Err(error) => error.to_string(),
        Ok(_) => String::new(),
    };
    assert!(matches!(
        &result,
        Err(OAuthError::TokenEndpointStatus {
            status: 400,
            error_code: Some(_),
            ..
        })
    ));
    assert!(!message.contains("secret-code"));
    assert!(!message.contains("secret-verifier"));
    Ok(())
}

fn test_config() -> Result<OAuthConfig, Box<dyn std::error::Error>> {
    Ok(OAuthConfig {
        auth_url: "https://auth.example.test/oauth/authorize".parse()?,
        token_url: "https://auth.example.test/oauth/token".parse()?,
        revoke_url: "https://auth.example.test/oauth/revoke".parse()?,
        client_id: "client-id".to_string(),
        scopes: vec!["openid".to_string(), "offline_access".to_string()],
        redirect_uri_template: "http://localhost:{port}/auth/callback".to_string(),
        callback_ports: vec![0],
        callback_timeout: Duration::from_secs(5),
        originator: Some("taugentic-test".to_string()),
        allowed_workspace_id: None,
    })
}

fn test_id_token() -> Result<String, Box<dyn std::error::Error>> {
    let payload = json!({
        "exp": 1_800_000_000_u64,
        "email": "user@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acc_123",
            "organization_id": "org_123",
            "chatgpt_plan_type": "plus",
            "chatgpt_account_is_fedramp": false
        }
    });
    let encoded_payload =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload)?);
    Ok(format!("e30.{encoded_payload}.sig"))
}

fn test_id_token_without_organization() -> Result<String, Box<dyn std::error::Error>> {
    let payload = json!({
        "exp": 1_800_000_000_u64,
        "email": "user@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acc_123",
            "chatgpt_plan_type": "plus",
            "chatgpt_account_is_fedramp": false
        }
    });
    let encoded_payload =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload)?);
    Ok(format!("e30.{encoded_payload}.sig"))
}

async fn send_callback(port: u16, target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
    let request =
        format!("GET {target} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n");
    tokio::io::AsyncWriteExt::write_all(&mut stream, request.as_bytes()).await?;
    let mut response = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut response).await?;
    Ok(())
}

#[derive(Clone)]
struct MockHttpClient {
    responses: Arc<Mutex<Vec<OAuthHttpResponse>>>,
    requests: Arc<Mutex<Vec<Vec<FormField>>>>,
}

impl MockHttpClient {
    fn new(mut responses: Vec<OAuthHttpResponse>) -> Self {
        responses.reverse();
        Self {
            responses: Arc::new(Mutex::new(responses)),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn requests(&self) -> Result<Vec<Vec<FormField>>, Box<dyn std::error::Error>> {
        let requests = self
            .requests
            .lock()
            .map_err(|_| "request lock poisoned")?
            .clone();
        Ok(requests)
    }
}

impl OAuthHttpClient for MockHttpClient {
    fn post_form<'a>(&'a self, _url: &'a url::Url, fields: &'a [FormField]) -> OAuthHttpFuture<'a> {
        Box::pin(async move {
            self.requests
                .lock()
                .map_err(|_| OAuthError::HttpTransport("request lock poisoned".to_string()))?
                .push(fields.to_vec());
            self.responses
                .lock()
                .map_err(|_| OAuthError::HttpTransport("response lock poisoned".to_string()))?
                .pop()
                .ok_or_else(|| OAuthError::HttpTransport("missing mock response".to_string()))
        })
    }
}
