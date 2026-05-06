use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefreshPolicy {
    pub proactive_window: Duration,
    pub max_consecutive_failures: u32,
    pub retry_backoff: Duration,
}

impl Default for RefreshPolicy {
    fn default() -> Self {
        Self {
            proactive_window: Duration::from_secs(5 * 60),
            max_consecutive_failures: 3,
            retry_backoff: Duration::from_secs(1),
        }
    }
}
