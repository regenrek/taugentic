use std::future::Future;
use std::pin::Pin;

use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::OAuthError;
use crate::flow::TokenSet;
use crate::oauth::redaction::{parse_token_endpoint_error, redact_oauth_error_text};
use crate::token::claims::parse_chatgpt_account_info;

pub type OAuthHttpFuture<'a> =
    Pin<Box<dyn Future<Output = Result<OAuthHttpResponse, OAuthError>> + Send + 'a>>;
pub type OAuthTokenFuture<'a> =
    Pin<Box<dyn Future<Output = Result<TokenSet, OAuthError>> + Send + 'a>>;
pub type OAuthUnitFuture<'a> = Pin<Box<dyn Future<Output = Result<(), OAuthError>> + Send + 'a>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormField {
    pub name: String,
    pub value: String,
}

impl FormField {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthHttpResponse {
    pub status: u16,
    pub body: String,
}

pub trait OAuthHttpClient: Send + Sync {
    fn post_form<'a>(&'a self, url: &'a Url, fields: &'a [FormField]) -> OAuthHttpFuture<'a>;

    fn refresh_token<'a>(
        &'a self,
        token_url: &'a Url,
        client_id: &'a str,
        refresh_token: &'a str,
    ) -> OAuthTokenFuture<'a> {
        Box::pin(async move {
            let fields = [
                FormField::new("client_id", client_id),
                FormField::new("grant_type", "refresh_token"),
                FormField::new("refresh_token", refresh_token),
            ];
            let response = self.post_form(token_url, &fields).await?;
            parse_token_set_response(response)
        })
    }

    fn revoke_token<'a>(
        &'a self,
        revoke_url: &'a Url,
        client_id: &'a str,
        token: &'a str,
    ) -> OAuthUnitFuture<'a> {
        Box::pin(async move {
            let fields = [
                FormField::new("client_id", client_id),
                FormField::new("token", token),
                FormField::new("token_type_hint", "refresh_token"),
            ];
            let response = self.post_form(revoke_url, &fields).await?;
            parse_unit_response(response)
        })
    }
}

#[derive(Clone)]
pub struct ReqwestOAuthHttpClient {
    inner: reqwest::Client,
}

impl ReqwestOAuthHttpClient {
    pub fn new(inner: reqwest::Client) -> Self {
        Self { inner }
    }
}

impl Default for ReqwestOAuthHttpClient {
    fn default() -> Self {
        Self {
            inner: reqwest::Client::new(),
        }
    }
}

impl OAuthHttpClient for ReqwestOAuthHttpClient {
    fn post_form<'a>(&'a self, url: &'a Url, fields: &'a [FormField]) -> OAuthHttpFuture<'a> {
        Box::pin(async move {
            let body = encode_form(fields);
            let response = self
                .inner
                .post(url.clone())
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(body)
                .send()
                .await
                .map_err(redacted_transport_error)?;

            let status = response.status().as_u16();
            let body = response.text().await.map_err(redacted_transport_error)?;
            Ok(OAuthHttpResponse { status, body })
        })
    }

    fn refresh_token<'a>(
        &'a self,
        token_url: &'a Url,
        client_id: &'a str,
        refresh_token: &'a str,
    ) -> OAuthTokenFuture<'a> {
        Box::pin(async move {
            let request = RefreshTokenRequest {
                client_id,
                grant_type: "refresh_token",
                refresh_token,
            };
            let body = serde_json::to_string(&request).map_err(OAuthError::TokenResponseJson)?;
            let response = self
                .inner
                .post(token_url.clone())
                .header(CONTENT_TYPE, "application/json")
                .body(body)
                .send()
                .await
                .map_err(redacted_transport_error)?;

            let status = response.status().as_u16();
            let body = response.text().await.map_err(redacted_transport_error)?;
            parse_token_set_response(OAuthHttpResponse { status, body })
        })
    }

    fn revoke_token<'a>(
        &'a self,
        revoke_url: &'a Url,
        client_id: &'a str,
        token: &'a str,
    ) -> OAuthUnitFuture<'a> {
        Box::pin(async move {
            let request = RevokeTokenRequest {
                client_id,
                token,
                token_type_hint: "refresh_token",
            };
            let body = serde_json::to_string(&request).map_err(OAuthError::TokenResponseJson)?;
            let response = self
                .inner
                .post(revoke_url.clone())
                .header(CONTENT_TYPE, "application/json")
                .body(body)
                .send()
                .await
                .map_err(redacted_transport_error)?;

            let status = response.status().as_u16();
            let body = response.text().await.map_err(redacted_transport_error)?;
            parse_unit_response(OAuthHttpResponse { status, body })
        })
    }
}

pub(crate) fn encode_form(fields: &[FormField]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for field in fields {
        serializer.append_pair(&field.name, &field.value);
    }
    serializer.finish()
}

fn redacted_transport_error(error: reqwest::Error) -> OAuthError {
    OAuthError::HttpTransport(format!(
        "request failed (timeout={}, connect={}, body={})",
        error.is_timeout(),
        error.is_connect(),
        error.is_body()
    ))
}

fn parse_token_set_response(response: OAuthHttpResponse) -> Result<TokenSet, OAuthError> {
    if !(200..=299).contains(&response.status) {
        return Err(status_error(response));
    }

    let parsed = serde_json::from_str::<RefreshTokenResponse>(&response.body)
        .map_err(OAuthError::TokenResponseJson)?;
    if parsed.access_token.is_empty() {
        return Err(OAuthError::MissingTokenField("access_token"));
    }
    if parsed.refresh_token.is_empty() {
        return Err(OAuthError::MissingTokenField("refresh_token"));
    }
    let account_info = match parsed.id_token.as_deref() {
        Some(id_token) => Some(parse_chatgpt_account_info(id_token)?),
        None => None,
    };
    Ok(TokenSet {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token,
        id_token: parsed.id_token,
        expires_in: parsed.expires_in,
        scope: parsed.scope,
        api_access_token: parsed.api_access_token,
        account_info,
    })
}

fn parse_unit_response(response: OAuthHttpResponse) -> Result<(), OAuthError> {
    if (200..=299).contains(&response.status) {
        return Ok(());
    }
    Err(status_error(response))
}

fn status_error(response: OAuthHttpResponse) -> OAuthError {
    let detail = parse_token_endpoint_error(&response.body);
    OAuthError::TokenEndpointStatus {
        status: response.status,
        error_code: detail.error_code,
        message: redact_oauth_error_text(&detail.message),
    }
}

#[derive(Serialize)]
struct RefreshTokenRequest<'a> {
    client_id: &'a str,
    grant_type: &'static str,
    refresh_token: &'a str,
}

#[derive(Serialize)]
struct RevokeTokenRequest<'a> {
    client_id: &'a str,
    token: &'a str,
    token_type_hint: &'static str,
}

#[derive(Debug, Deserialize)]
struct RefreshTokenResponse {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    api_access_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{FormField, OAuthHttpResponse, encode_form, parse_token_set_response};

    #[test]
    fn encode_form_escapes_values() {
        let fields = [
            FormField::new("grant_type", "authorization_code"),
            FormField::new("redirect_uri", "http://localhost:1455/auth/callback"),
            FormField::new("code", "a b+c"),
        ];

        assert_eq!(
            encode_form(&fields),
            "grant_type=authorization_code&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&code=a+b%2Bc"
        );
    }

    #[test]
    fn parse_refresh_response_redacts_status_error() {
        let secret = "fake-refresh-token";
        let response = OAuthHttpResponse {
            status: 401,
            body: json!({
                "error": "invalid_grant",
                "error_description": format!("refresh token rejected refresh_token={secret}&access_token=fake-access")
            })
            .to_string(),
        };

        let error = parse_token_set_response(response).expect_err("status should fail");

        let display = error.to_string();
        assert!(display.contains("refresh_token=<redacted>"));
        assert!(display.contains("access_token=<redacted>"));
        assert!(!display.contains(secret));
        assert!(!display.contains("fake-access"));
    }
}
