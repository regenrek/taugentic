use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkSourceError {
    #[error("work source request was cancelled")]
    Cancelled,
    #[error("work source configuration is invalid: {0}")]
    InvalidConfig(String),
    #[error("work source credentials are missing")]
    CredentialsMissing,
    #[error("work source authentication failed")]
    Authentication,
    #[error("work source permission was denied")]
    PermissionDenied,
    #[error("work source request was rate limited")]
    RateLimited { retry_after: Option<Duration> },
    #[error("work source service is unavailable")]
    Unavailable,
    #[error("work source response is invalid: {0}")]
    InvalidResponse(String),
}
