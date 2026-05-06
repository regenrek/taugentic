pub mod anthropic;
pub mod openai;
pub mod openai_responses;

#[derive(Debug, thiserror::Error)]
pub enum ProviderStreamError {
    #[error("invalid stream JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Failure(ProviderStreamFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderStreamFailure {
    pub code: Option<String>,
    pub message: String,
}

impl std::fmt::Display for ProviderStreamFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.code.as_deref() {
            Some(code) if !code.is_empty() => write!(formatter, "{code}: {}", self.message),
            _ => formatter.write_str(&self.message),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStreamEvent {
    AssistantTextDelta(String),
    ToolCallStarted { index: u64, name: String },
    ToolCallProgress { index: u64, delta: String },
    ToolCallCompleted { index: u64 },
    ToolCallBatchCompleted,
    TokenUsage(ProviderTokenUsage),
    TurnCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub model: Option<String>,
}
