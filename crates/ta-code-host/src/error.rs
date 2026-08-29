use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodeHostError {
    #[error("code host request was cancelled")]
    Cancelled,
    #[error("code host configuration is invalid")]
    InvalidConfig,
    #[error("code host input is invalid")]
    InvalidInput,
    #[error("code host credentials are missing")]
    CredentialsMissing,
    #[error("code host credential storage is unavailable")]
    CredentialsBackend,
    #[error("code host authentication failed")]
    Unauthorized,
    #[error("code host permission was denied")]
    Forbidden,
    #[error("code host resource was not found")]
    NotFound,
    #[error("code host resource conflicts with current state")]
    Conflict,
    #[error("code host request was rate limited")]
    RateLimited { retry_after: Option<Duration> },
    #[error("code host request was rejected")]
    Validation,
    #[error("code host service is unavailable")]
    Unavailable,
    #[error("the result of the code host mutation is unknown")]
    OutcomeUnknown,
    #[error("code host response exceeded its production bound")]
    ResponseTooLarge,
    #[error("code host response was invalid")]
    InvalidResponse,
}
