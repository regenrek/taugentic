use crate::error::LlmClientError;
use ta_protocol::wire::{AuthProfileLoginResult, AuthProfileLogoutResult};
use thiserror::Error;

pub const CODEX_PROVIDER_ID: &str = "codex";

#[derive(Debug, Clone, Error)]
pub enum CodexLlmClientError {
    #[error("codex auth profile does not exist: {0}")]
    UnknownAuthProfile(String),
    #[error("codex CLI is not available: {0}")]
    CliUnavailable(String),
    #[error("codex command timed out: {0}")]
    CommandTimedOut(String),
    #[error("codex command failed: {0}")]
    CommandFailed(String),
    #[error("codex auth failed: {0}")]
    Auth(String),
    #[error("codex app-server rate limited: {detail}")]
    RateLimited {
        retry_after_ms: Option<u64>,
        detail: String,
    },
    #[error("codex credits exhausted: {0}")]
    CreditsExhausted(String),
    #[error("codex context length exceeded: {0}")]
    ContextLengthExceeded(String),
    #[error("codex app-server config is invalid: {0}")]
    InvalidConfig(String),
    #[error("codex app-server protocol error: {0}")]
    Protocol(String),
    #[error("codex app-server JSON-RPC error {code}: {message}")]
    JsonRpc {
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },
    #[error("codex app-server execution cancelled: {0}")]
    Cancelled(String),
}

impl From<CodexLlmClientError> for LlmClientError {
    fn from(error: CodexLlmClientError) -> Self {
        match error {
            CodexLlmClientError::UnknownAuthProfile(message) => {
                LlmClientError::InvalidConfig(message)
            }
            CodexLlmClientError::CliUnavailable(message)
            | CodexLlmClientError::CommandTimedOut(message)
            | CodexLlmClientError::CommandFailed(message)
            | CodexLlmClientError::Protocol(message) => LlmClientError::ProcessFailed(message),
            CodexLlmClientError::Auth(message) => LlmClientError::Auth(message),
            CodexLlmClientError::RateLimited {
                retry_after_ms,
                detail,
            } => LlmClientError::RateLimited {
                retry_after_ms,
                detail,
            },
            CodexLlmClientError::CreditsExhausted(message) => {
                LlmClientError::CreditsExhausted(message)
            }
            CodexLlmClientError::ContextLengthExceeded(message) => {
                LlmClientError::ContextLengthExceeded(message)
            }
            CodexLlmClientError::InvalidConfig(message) => LlmClientError::InvalidConfig(message),
            CodexLlmClientError::JsonRpc { code, message, .. } => {
                if code == -32001 {
                    LlmClientError::RateLimited {
                        retry_after_ms: None,
                        detail: message,
                    }
                } else {
                    LlmClientError::ServerError(message)
                }
            }
            CodexLlmClientError::Cancelled(message) => LlmClientError::Cancelled(message),
        }
    }
}

pub type CodexLoginResult = Result<AuthProfileLoginResult, CodexLlmClientError>;
pub type CodexLogoutResult = Result<AuthProfileLogoutResult, CodexLlmClientError>;
