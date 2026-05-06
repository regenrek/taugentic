use std::time::Duration;

use reqwest::{StatusCode, header};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitBackoff {
    pub retry_after: Duration,
    pub reason: RateLimitReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitReason {
    RetryAfter,
    PrimaryReset,
    Secondary,
}

#[derive(Debug, Error)]
pub enum WorkSourceError {
    #[error("work source request was cancelled")]
    Cancelled,
    #[error("work source configuration is invalid: {0}")]
    InvalidConfig(String),
    #[error("GitHub credentials are missing")]
    CredentialsMissing,
    #[error("GitHub credentials backend failed: {0}")]
    CredentialsBackend(String),
    #[error("GitHub response schema is invalid: {0}")]
    InvalidResponse(String),
    #[error("GitHub request failed with status {status}")]
    HttpStatus {
        status: StatusCode,
        backoff: Option<RateLimitBackoff>,
    },
    #[error("GitHub request failed: {0}")]
    Network(#[from] reqwest::Error),
}

pub(crate) fn rate_limit_backoff(headers: &header::HeaderMap) -> Option<RateLimitBackoff> {
    retry_after(headers)
        .map(|retry_after| RateLimitBackoff {
            retry_after,
            reason: RateLimitReason::RetryAfter,
        })
        .or_else(|| primary_reset(headers))
}

fn retry_after(headers: &header::HeaderMap) -> Option<Duration> {
    let value = headers.get(header::RETRY_AFTER)?.to_str().ok()?;
    let seconds = value.trim().parse::<u64>().ok()?;
    Some(Duration::from_secs(seconds.max(1)))
}

fn primary_reset(headers: &header::HeaderMap) -> Option<RateLimitBackoff> {
    let remaining = headers
        .get("x-ratelimit-remaining")?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    if remaining != 0 {
        return None;
    }
    let reset_epoch_seconds = headers
        .get("x-ratelimit-reset")?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    let now_epoch_seconds = unix_epoch_seconds();
    let wait_seconds = reset_epoch_seconds.saturating_sub(now_epoch_seconds).max(1);
    Some(RateLimitBackoff {
        retry_after: Duration::from_secs(wait_seconds),
        reason: RateLimitReason::PrimaryReset,
    })
}

fn unix_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
