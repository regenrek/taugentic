use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("invalid OAuth configuration: {0}")]
    InvalidConfig(String),

    #[error("invalid URL in {field}")]
    InvalidUrl {
        field: &'static str,
        #[source]
        source: url::ParseError,
    },

    #[error("failed to bind OAuth callback server on configured ports {ports:?}")]
    CallbackBindFailed {
        ports: Vec<u16>,
        #[source]
        source: io::Error,
    },

    #[error("OAuth callback timed out")]
    CallbackTimeout,

    #[error("OAuth callback state did not match the expected value")]
    StateMismatch,

    #[error("OAuth callback did not include an authorization code")]
    MissingAuthorizationCode,

    #[error("OAuth provider returned an authorization error: {code}")]
    AuthorizationError {
        code: String,
        description: Option<String>,
    },

    #[error("OAuth HTTP transport failed: {0}")]
    HttpTransport(String),

    #[error("OAuth token endpoint returned status {status}: {message}")]
    TokenEndpointStatus {
        status: u16,
        error_code: Option<String>,
        message: String,
    },

    #[error("OAuth token response is missing required field `{0}`")]
    MissingTokenField(&'static str),

    #[error("OAuth token response could not be parsed")]
    TokenResponseJson(#[source] serde_json::Error),

    #[error("JWT claims could not be parsed: {0}")]
    InvalidJwt(String),

    #[error("I/O error while handling OAuth callback")]
    Io(#[from] io::Error),
}
