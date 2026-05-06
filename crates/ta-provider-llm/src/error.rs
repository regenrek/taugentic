use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LlmClientError {
    #[error("authentication failed: {0}")]
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
    #[error("credits exhausted: {0}")]
    CreditsExhausted(String),
    #[error("context length exceeded: {0}")]
    ContextLengthExceeded(String),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("process failed: {0}")]
    ProcessFailed(String),
    #[error("cancelled: {0}")]
    Cancelled(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("server error: {0}")]
    ServerError(String),
}
