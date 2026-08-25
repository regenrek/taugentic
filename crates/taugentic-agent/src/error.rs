use ta_protocol::wire::{ApprovalScope, WorkspaceCapabilityUnsupported};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    #[error("auth failed: {0}")]
    Auth(String),
    #[error("credentials missing: {0}")]
    CredentialsMissing(String),
    #[error(
        "feature requires OpenAI Platform organization: connect a Platform organization to the ChatGPT subscription account"
    )]
    FeatureRequiresPlatformOrg,
    #[error(
        "This OpenAI runtime profile uses an OpenAI Platform API key. Select an `OpenAI ChatGPT *` runtime profile to use a ChatGPT subscription, or configure OPENAI_API_KEY."
    )]
    SubscriptionAuthIncompatibleWithNativeClient,
    #[error("network error: {0}")]
    Network(String),
    #[error("rate limited: {detail}")]
    RateLimited {
        retry_after_ms: Option<u64>,
        detail: String,
    },
    #[error("provider credits exhausted: {0}")]
    CreditsExhausted(String),
    #[error("context length exceeded: {0}")]
    ContextLengthExceeded(String),
    #[error("invalid provider config: {0}")]
    InvalidConfig(String),
    #[error("provider process failed: {0}")]
    ProcessFailed(String),
    #[error("provider execution cancelled: {0}")]
    Cancelled(String),
    #[error("unsupported provider operation: {0}")]
    Unsupported(String),
    #[error("execution policy denied {scope:?}: {reason}")]
    PolicyDenied {
        scope: ApprovalScope,
        reason: String,
    },
    #[error("{0}")]
    WorkspaceCapabilityUnsupported(WorkspaceCapabilityUnsupported),
    #[error("provider server error: {0}")]
    ServerError(String),
    #[error("invalid tool input: {0}")]
    InvalidToolInput(String),
    #[error("tool failed: {0}")]
    ToolFailed(String),
    #[error("tool list locked: {0}")]
    ToolListLocked(String),
    #[error("incomplete continuation exhausted after {attempts} attempts")]
    IncompleteContinuation { attempts: usize },
    #[error("process timed out after {timeout_ms}ms: {detail}")]
    ProcessTimeout { timeout_ms: u64, detail: String },
}

impl From<ta_provider_llm::error::LlmClientError> for ExecutionError {
    fn from(error: ta_provider_llm::error::LlmClientError) -> Self {
        use ta_provider_llm::error::LlmClientError;

        match error {
            LlmClientError::Auth(detail) => Self::Auth(detail),
            LlmClientError::CredentialsMissing(detail) => Self::CredentialsMissing(detail),
            LlmClientError::FeatureRequiresPlatformOrg => Self::FeatureRequiresPlatformOrg,
            LlmClientError::SubscriptionAuthIncompatibleWithNativeClient => {
                Self::SubscriptionAuthIncompatibleWithNativeClient
            }
            LlmClientError::Network(detail) => Self::Network(detail),
            LlmClientError::RateLimited {
                retry_after_ms,
                detail,
            } => Self::RateLimited {
                retry_after_ms,
                detail,
            },
            LlmClientError::CreditsExhausted(detail) => Self::CreditsExhausted(detail),
            LlmClientError::ContextLengthExceeded(detail) => Self::ContextLengthExceeded(detail),
            LlmClientError::InvalidConfig(detail) => Self::InvalidConfig(detail),
            LlmClientError::ProcessFailed(detail) => Self::ProcessFailed(detail),
            LlmClientError::Cancelled(detail) => Self::Cancelled(detail),
            LlmClientError::Unsupported(detail) => Self::Unsupported(detail),
            LlmClientError::ServerError(detail) => Self::ServerError(detail),
        }
    }
}

impl From<ta_provider_acp::error::AcpClientError> for ExecutionError {
    fn from(error: ta_provider_acp::error::AcpClientError) -> Self {
        use ta_provider_acp::error::AcpClientError;

        match error {
            AcpClientError::WorkspaceCapabilityUnsupported(detail) => {
                Self::WorkspaceCapabilityUnsupported(detail)
            }
            AcpClientError::InvalidConfig(detail) => Self::InvalidConfig(detail),
            AcpClientError::ProcessFailed(detail) => Self::ProcessFailed(detail),
            AcpClientError::JsonRpcRequestFailed { .. } => Self::ProcessFailed(error.to_string()),
            AcpClientError::Cancelled(detail) => Self::Cancelled(detail),
            AcpClientError::JsonRpc { code, message } => {
                Self::ProcessFailed(format!("ACP JSON-RPC error {code}: {message}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acp_capability_error_remains_typed_at_the_agent_boundary() {
        let detail = WorkspaceCapabilityUnsupported {
            variant: None,
            vendor: Some("cursor".to_string()),
            capability: "network".to_string(),
            requested: "none".to_string(),
            reason: "provider cannot separate model and tool network".to_string(),
        };

        assert_eq!(
            ExecutionError::from(
                ta_provider_acp::error::AcpClientError::WorkspaceCapabilityUnsupported(
                    detail.clone(),
                ),
            ),
            ExecutionError::WorkspaceCapabilityUnsupported(detail)
        );
    }
}
