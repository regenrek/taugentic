use crate::error::LlmClientError;
use ta_protocol::wire::{
    AgentRuntimeStrategyInfo, AuthProfileId, AuthProfileLoginResult, AuthProfileLogoutResult,
    AuthProfileState,
};
use thiserror::Error;

pub const CODEX_PROVIDER_ID: &str = "codex";
pub const CODEX_DEFAULT_MODEL_ID: &str = "gpt-5.4";
pub const CODEX_CHATGPT_AUTH_PROFILE_ID: &str = "auth-codex-chatgpt";
pub const CODEX_API_KEY_AUTH_PROFILE_ID: &str = "auth-codex-api-key";
pub const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexAuthMode {
    Chatgpt,
    ApiKey,
    LoggedOut,
    Unknown(String),
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexProviderSnapshot {
    pub provider: AgentRuntimeStrategyInfo,
    pub auth_profiles: Vec<AuthProfileState>,
}

#[derive(Debug, Error)]
pub enum CodexLlmClientError {
    #[error("codex auth profile does not exist: {0}")]
    UnknownAuthProfile(String),
    #[error("codex CLI is not available: {0}")]
    CliUnavailable(String),
    #[error("codex command timed out: {0}")]
    CommandTimedOut(String),
    #[error("codex login with API key requires {OPENAI_API_KEY_ENV_VAR} in the daemon environment")]
    MissingApiKeyEnv,
    #[error("codex command failed: {0}")]
    CommandFailed(String),
    #[error("codex login did not authenticate the requested profile")]
    LoginDidNotAuthenticate,
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
            CodexLlmClientError::MissingApiKeyEnv => LlmClientError::CredentialsMissing(format!(
                "codex login with API key requires {OPENAI_API_KEY_ENV_VAR}"
            )),
            CodexLlmClientError::LoginDidNotAuthenticate => LlmClientError::Auth(
                "codex login did not authenticate the requested profile".into(),
            ),
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

pub fn matches_auth_profile_id(auth_profile_id: &AuthProfileId) -> bool {
    matches!(
        auth_profile_id.as_str(),
        CODEX_CHATGPT_AUTH_PROFILE_ID | CODEX_API_KEY_AUTH_PROFILE_ID
    )
}
