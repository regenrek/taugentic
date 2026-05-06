use std::time::Duration;

pub const DEFAULT_WAIT_TIMEOUT_MS: u64 = 5_000;
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 250;
pub const DEFAULT_WATCH_INTERVAL_MS: u64 = 1_000;
pub const DEFAULT_LOG_TAIL_LINES: usize = 200;

pub fn default_wait_timeout() -> Duration {
    Duration::from_millis(DEFAULT_WAIT_TIMEOUT_MS)
}

pub fn default_poll_interval() -> Duration {
    Duration::from_millis(DEFAULT_POLL_INTERVAL_MS)
}
