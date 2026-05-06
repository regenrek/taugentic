//! Canonical provider id validation shared by runtime provider integrations.

use thiserror::Error;

/// Stable validation rule for provider ids that are also cache path segments.
pub const PROVIDER_ID_SEGMENT_RULE: &str = "must match [a-z0-9][a-z0-9_-]*";

/// Shared valid provider-id cases used by provider crate regression tests.
#[doc(hidden)]
pub const VALID_PROVIDER_ID_TEST_CASES: &[&str] = &[
    "codex-acp",
    "claude-acp",
    "cursor",
    "opencode",
    "copilot-acp",
    "gemini",
    "custom_1",
];

/// Shared invalid provider-id cases used by provider crate regression tests.
#[doc(hidden)]
pub const INVALID_PROVIDER_ID_TEST_CASES: &[&str] = &[
    "",
    "../escape",
    "escape/child",
    "escape\\child",
    ".",
    "..",
    "Upper",
    "has.dot",
    "-leading",
    "_leading",
    "with space",
];

/// Error raised when a provider id does not match the canonical segment contract.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("invalid provider id {provider_id:?}: {reason}")]
pub struct ProviderIdError {
    provider_id: String,
    reason: &'static str,
}

impl ProviderIdError {
    /// Invalid provider id supplied by the caller.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Stable validation reason.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

/// Validates the canonical provider id/cache segment contract.
pub fn validate_provider_id(provider_id: &str) -> Result<(), ProviderIdError> {
    let Some(first) = provider_id.bytes().next() else {
        return Err(invalid_provider_id(provider_id));
    };
    if !is_provider_id_start(first) || !provider_id.bytes().skip(1).all(is_provider_id_rest) {
        return Err(invalid_provider_id(provider_id));
    }
    Ok(())
}

fn invalid_provider_id(provider_id: &str) -> ProviderIdError {
    ProviderIdError {
        provider_id: provider_id.to_string(),
        reason: PROVIDER_ID_SEGMENT_RULE,
    }
}

fn is_provider_id_start(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit()
}

fn is_provider_id_rest(byte: u8) -> bool {
    is_provider_id_start(byte) || matches!(byte, b'_' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_ids_match_canonical_segment_contract() {
        for valid in VALID_PROVIDER_ID_TEST_CASES {
            validate_provider_id(valid).expect("valid provider id");
        }
        for invalid in INVALID_PROVIDER_ID_TEST_CASES {
            assert!(validate_provider_id(invalid).is_err());
        }
    }
}
