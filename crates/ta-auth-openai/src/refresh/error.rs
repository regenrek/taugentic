use thiserror::Error;

use crate::{CredentialStoreError, OAuthError};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TokenRefreshError {
    #[error("no OpenAI ChatGPT credentials are stored")]
    NoCredentials,

    #[error("stored OpenAI ChatGPT credentials do not include a refresh token")]
    NoRefreshToken,

    #[error("OpenAI ChatGPT token refresh request failed")]
    NetworkError { status: Option<u16> },

    #[error("OpenAI ChatGPT authorization was revoked; sign in again")]
    AuthRevoked,

    #[error("OpenAI ChatGPT access token is expired or has no usable expiry")]
    TokenExpired,

    #[error("OpenAI ChatGPT token refresh exceeded retry policy after {attempts} attempts")]
    MaxRetriesExceeded { attempts: u32 },

    #[error("OpenAI ChatGPT credential backend is unavailable during {operation}")]
    BackendUnavailable { operation: &'static str },
}

impl TokenRefreshError {
    pub(crate) fn from_store(
        _error: CredentialStoreError,
        operation: &'static str,
    ) -> TokenRefreshError {
        TokenRefreshError::BackendUnavailable { operation }
    }

    pub(crate) fn from_oauth(error: OAuthError) -> TokenRefreshError {
        match error {
            OAuthError::TokenEndpointStatus { status, .. } => TokenRefreshError::NetworkError {
                status: Some(status),
            },
            OAuthError::HttpTransport(_)
            | OAuthError::TokenResponseJson(_)
            | OAuthError::MissingTokenField(_)
            | OAuthError::InvalidJwt(_) => TokenRefreshError::NetworkError { status: None },
            OAuthError::InvalidConfig(_)
            | OAuthError::InvalidUrl { .. }
            | OAuthError::CallbackBindFailed { .. }
            | OAuthError::CallbackTimeout
            | OAuthError::StateMismatch
            | OAuthError::MissingAuthorizationCode
            | OAuthError::AuthorizationError { .. }
            | OAuthError::Io(_) => TokenRefreshError::BackendUnavailable {
                operation: "oauth-refresh",
            },
        }
    }
}
