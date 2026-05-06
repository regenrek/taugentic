pub const DEFAULT_APPROVAL_TTL_MS: u64 = 10 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApprovalTtlPolicy {
    ttl_ms: u64,
}

impl ApprovalTtlPolicy {
    pub const fn default_interactive() -> Self {
        Self {
            ttl_ms: DEFAULT_APPROVAL_TTL_MS,
        }
    }

    pub const fn ttl_ms(self) -> u64 {
        self.ttl_ms
    }

    pub fn expires_at_ms(self, requested_at_ms: u64) -> u64 {
        requested_at_ms.saturating_add(self.ttl_ms)
    }

    pub fn is_expired(self, expires_at_ms: u64, now_ms: u64) -> bool {
        now_ms >= expires_at_ms
    }
}

impl Default for ApprovalTtlPolicy {
    fn default() -> Self {
        Self::default_interactive()
    }
}
