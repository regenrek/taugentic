use std::time::Duration;

use crate::error::LlmClientError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(4),
        }
    }
}

pub fn should_retry(error: &LlmClientError) -> bool {
    matches!(
        error,
        LlmClientError::Network(_)
            | LlmClientError::RateLimited { .. }
            | LlmClientError::ServerError(_)
    )
}

pub fn retry_delay(policy: RetryPolicy, attempt_index: usize, error: &LlmClientError) -> Duration {
    match error {
        LlmClientError::RateLimited {
            retry_after_ms: Some(retry_after_ms),
            ..
        } => Duration::from_millis(*retry_after_ms),
        _ => backoff_delay(policy, attempt_index),
    }
}

pub fn backoff_delay(policy: RetryPolicy, attempt_index: usize) -> Duration {
    let multiplier = 1u32.checked_shl(attempt_index as u32).unwrap_or(u32::MAX);
    policy
        .base_delay
        .saturating_mul(multiplier)
        .min(policy.max_delay)
}
