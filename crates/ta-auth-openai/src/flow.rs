use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

use crate::client::{FormField, OAuthHttpClient, OAuthHttpResponse};
use crate::config::OAuthConfig;
use crate::error::OAuthError;
use crate::oauth::pkce::PkcePair;
use crate::oauth::redaction::{parse_token_endpoint_error, redact_oauth_error_text};
use crate::oauth::server::{CallbackServer, CallbackServerConfig, start_callback_server};
use crate::token::claims::{ChatGptAccountInfo, parse_chatgpt_account_info};

pub struct OpenAiOAuthFlow;

pub struct CompletionHandle {
    callback_server: CallbackServer,
    code_verifier: String,
    token_url: Url,
    client_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthCode {
    pub code: String,
    pub code_verifier: String,
    pub redirect_uri: String,
    token_url: Url,
    client_id: String,
}

impl OAuthCode {
    pub fn new(
        code: impl Into<String>,
        code_verifier: impl Into<String>,
        redirect_uri: impl Into<String>,
        config: &OAuthConfig,
    ) -> Self {
        Self {
            code: code.into(),
            code_verifier: code_verifier.into(),
            redirect_uri: redirect_uri.into(),
            token_url: config.token_url.clone(),
            client_id: config.client_id.clone(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: Option<String>,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
    pub api_access_token: Option<String>,
    pub account_info: Option<ChatGptAccountInfo>,
}

impl fmt::Debug for TokenSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokenSet")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("id_token", &self.id_token.as_ref().map(|_| "<redacted>"))
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .field(
                "api_access_token",
                &self.api_access_token.as_ref().map(|_| "<redacted>"),
            )
            .field("account_info", &self.account_info)
            .finish()
    }
}

impl TokenSet {
    pub fn expires_at(&self, issued_at: SystemTime) -> Option<SystemTime> {
        self.expires_in
            .and_then(|expires_in| issued_at.checked_add(Duration::from_secs(expires_in)))
            .or_else(|| {
                self.account_info
                    .as_ref()
                    .and_then(|info| info.expires_at)
                    .map(|expires_at| UNIX_EPOCH + Duration::from_secs(expires_at))
            })
    }

    pub fn is_expired_or_within(&self, issued_at: SystemTime, window: Duration) -> bool {
        let Some(expires_at) = self.expires_at(issued_at) else {
            return true;
        };
        match SystemTime::now().checked_add(window) {
            Some(refresh_at) => expires_at <= refresh_at,
            None => true,
        }
    }
}

impl OpenAiOAuthFlow {
    pub async fn start(config: OAuthConfig) -> Result<(Url, CompletionHandle), OAuthError> {
        validate_config(&config)?;
        let pkce = PkcePair::generate();
        let state = generate_state();
        let callback_config = CallbackServerConfig::from_oauth_config(&config, state.clone());
        let callback_server = start_callback_server(callback_config).await?;
        let redirect_uri = callback_server.redirect_uri().to_string();
        let authorize_url = build_authorize_url(&config, &redirect_uri, &pkce, &state);

        Ok((
            authorize_url,
            CompletionHandle {
                callback_server,
                code_verifier: pkce.verifier,
                token_url: config.token_url,
                client_id: config.client_id,
            },
        ))
    }

    pub async fn exchange_code<C>(
        http_client: &C,
        oauth_code: OAuthCode,
    ) -> Result<TokenSet, OAuthError>
    where
        C: OAuthHttpClient + ?Sized,
    {
        let token_response = post_authorization_code(http_client, &oauth_code).await?;
        let account_info = match token_response.id_token.as_deref() {
            Some(id_token) => Some(parse_chatgpt_account_info(id_token)?),
            None => None,
        };
        let api_access_token = match token_response.id_token.as_deref() {
            Some(id_token) => {
                match post_api_token_exchange(http_client, &oauth_code, id_token).await {
                    Ok(token) => Some(token),
                    Err(error) if is_missing_organization_token_exchange_error(&error) => {
                        tracing::warn!(
                            error = %error,
                            "OpenAI ChatGPT API token exchange skipped because the ID token lacks organization_id; continuing with subscription-only auth"
                        );
                        None
                    }
                    Err(error) => return Err(error),
                }
            }
            None => None,
        };

        Ok(TokenSet {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            id_token: token_response.id_token,
            expires_in: token_response.expires_in,
            scope: token_response.scope,
            api_access_token,
            account_info,
        })
    }
}

impl CompletionHandle {
    pub fn callback_addr(&self) -> SocketAddr {
        self.callback_server.local_addr()
    }

    pub async fn await_code(self) -> Result<OAuthCode, OAuthError> {
        let redirect_uri = self.callback_server.redirect_uri().to_string();
        let code = self.callback_server.wait_for_code().await?;
        Ok(OAuthCode {
            code,
            code_verifier: self.code_verifier,
            redirect_uri,
            token_url: self.token_url,
            client_id: self.client_id,
        })
    }
}

fn validate_config(config: &OAuthConfig) -> Result<(), OAuthError> {
    if config.client_id.trim().is_empty() {
        return Err(OAuthError::InvalidConfig(
            "client_id must not be empty".to_string(),
        ));
    }
    if config.scopes.is_empty() {
        return Err(OAuthError::InvalidConfig(
            "at least one OAuth scope is required".to_string(),
        ));
    }
    if config.callback_ports.is_empty() {
        return Err(OAuthError::InvalidConfig(
            "callback_ports must not be empty".to_string(),
        ));
    }
    config.build_redirect_uri(config.callback_ports[0])?;
    Ok(())
}

fn build_authorize_url(
    config: &OAuthConfig,
    redirect_uri: &str,
    pkce: &PkcePair,
    state: &str,
) -> Url {
    let mut authorize_url = config.auth_url.clone();
    {
        let mut query = authorize_url.query_pairs_mut();
        query.append_pair("response_type", "code");
        query.append_pair("client_id", &config.client_id);
        query.append_pair("redirect_uri", redirect_uri);
        query.append_pair("scope", &config.scope_value());
        query.append_pair("code_challenge", &pkce.challenge);
        query.append_pair("code_challenge_method", pkce.method);
        query.append_pair("id_token_add_organizations", "true");
        query.append_pair("codex_cli_simplified_flow", "true");
        query.append_pair("state", state);
        if let Some(originator) = config.authorize_originator() {
            query.append_pair("originator", originator);
        }
        if let Some(workspace_id) = &config.allowed_workspace_id {
            query.append_pair("allowed_workspace_id", workspace_id);
        }
    }
    authorize_url
}

fn generate_state() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

async fn post_authorization_code<C>(
    http_client: &C,
    oauth_code: &OAuthCode,
) -> Result<AuthorizationCodeTokenResponse, OAuthError>
where
    C: OAuthHttpClient + ?Sized,
{
    let fields = [
        FormField::new("grant_type", "authorization_code"),
        FormField::new("code", &oauth_code.code),
        FormField::new("redirect_uri", &oauth_code.redirect_uri),
        FormField::new("client_id", &oauth_code.client_id),
        FormField::new("code_verifier", &oauth_code.code_verifier),
    ];
    let response = http_client
        .post_form(&oauth_code.token_url, &fields)
        .await?;
    parse_response(response)
}

async fn post_api_token_exchange<C>(
    http_client: &C,
    oauth_code: &OAuthCode,
    id_token: &str,
) -> Result<String, OAuthError>
where
    C: OAuthHttpClient + ?Sized,
{
    let fields = [
        FormField::new(
            "grant_type",
            "urn:ietf:params:oauth:grant-type:token-exchange",
        ),
        FormField::new("client_id", &oauth_code.client_id),
        FormField::new("requested_token", "openai-api-key"),
        FormField::new("subject_token", id_token),
        FormField::new(
            "subject_token_type",
            "urn:ietf:params:oauth:token-type:id_token",
        ),
    ];
    let response = http_client
        .post_form(&oauth_code.token_url, &fields)
        .await?;
    if !(200..=299).contains(&response.status) {
        return Err(status_error(response));
    }
    let parsed = serde_json::from_str::<ApiTokenExchangeResponse>(&response.body)
        .map_err(OAuthError::TokenResponseJson)?;
    if parsed.access_token.is_empty() {
        return Err(OAuthError::MissingTokenField("access_token"));
    }
    Ok(parsed.access_token)
}

fn parse_response(
    response: OAuthHttpResponse,
) -> Result<AuthorizationCodeTokenResponse, OAuthError> {
    if !(200..=299).contains(&response.status) {
        return Err(status_error(response));
    }

    let parsed = serde_json::from_str::<AuthorizationCodeTokenResponse>(&response.body)
        .map_err(OAuthError::TokenResponseJson)?;
    if parsed.access_token.is_empty() {
        return Err(OAuthError::MissingTokenField("access_token"));
    }
    if parsed.refresh_token.is_empty() {
        return Err(OAuthError::MissingTokenField("refresh_token"));
    }
    Ok(parsed)
}

fn status_error(response: OAuthHttpResponse) -> OAuthError {
    let detail = parse_token_endpoint_error(&response.body);
    OAuthError::TokenEndpointStatus {
        status: response.status,
        error_code: detail.error_code,
        message: redact_oauth_error_text(&detail.message),
    }
}

fn is_missing_organization_token_exchange_error(error: &OAuthError) -> bool {
    match error {
        OAuthError::TokenEndpointStatus {
            status: 401,
            message,
            ..
        } => {
            // OpenAI currently exposes this as a freeform 401 message. Keep the
            // matcher tight until a structured error code is available.
            message.contains("missing organization_id")
        }
        _ => false,
    }
}

#[derive(Debug, Deserialize)]
struct AuthorizationCodeTokenResponse {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiTokenExchangeResponse {
    access_token: String,
}
