use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::{
    EventParser, HttpLlmStream, LlmClient, LlmStream, StopReason, StreamEvent, StreamMessage,
    StreamRequest, StreamRole, StreamTool, require_stream_response, send_request,
};
use crate::error::LlmClientError;
use crate::families::anthropic::ANTHROPIC_API_KEY_ENV_VAR;
use crate::formats::anthropic::{self, AnthropicStreamEvent};
use crate::http::shared_client;
use tracing::instrument;

const ANTHROPIC_MESSAGES_BASE_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Clone)]
pub struct AnthropicMessagesClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl AnthropicMessagesClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, LlmClientError> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(LlmClientError::CredentialsMissing(
                "Anthropic API key is empty".to_string(),
            ));
        }
        let model = model.into();
        if model.trim().is_empty() {
            return Err(LlmClientError::InvalidConfig(
                "Anthropic Messages model is empty".to_string(),
            ));
        }
        Ok(Self {
            http: shared_client(),
            base_url: trim_base_url(base_url.into()),
            api_key,
            model,
        })
    }

    pub fn from_env(model: impl Into<String>) -> Result<Self, LlmClientError> {
        let api_key = std::env::var(ANTHROPIC_API_KEY_ENV_VAR).map_err(|_| {
            LlmClientError::CredentialsMissing(format!("{ANTHROPIC_API_KEY_ENV_VAR} is not set"))
        })?;
        Self::new(ANTHROPIC_MESSAGES_BASE_URL, api_key, model)
    }

    async fn send(
        &self,
        request: StreamRequest,
        cancellation: &CancellationToken,
    ) -> Result<reqwest::Response, LlmClientError> {
        let body = serde_json::to_vec(&messages_body(&request, &self.model))
            .map_err(|error| LlmClientError::InvalidConfig(error.to_string()))?;
        let request = self
            .http
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body);
        let response = send_request(request, cancellation).await?;
        require_stream_response(response, cancellation).await
    }
}

#[async_trait]
impl LlmClient for AnthropicMessagesClient {
    #[instrument(level = "debug", skip_all, fields(provider = "anthropic_messages"))]
    async fn start_stream(
        &self,
        request: StreamRequest,
        cancellation: CancellationToken,
    ) -> Result<Box<dyn LlmStream>, LlmClientError> {
        if cancellation.is_cancelled() {
            return Err(LlmClientError::Cancelled(
                "stream cancelled before request".to_string(),
            ));
        }
        let response = self.send(request, &cancellation).await?;
        Ok(Box::new(HttpLlmStream::new(
            response,
            parse_event as EventParser,
            cancellation,
        )))
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }
}

fn messages_body(request: &StreamRequest, model: &str) -> serde_json::Value {
    let system = request
        .messages
        .iter()
        .filter(|message| matches!(message.role, StreamRole::System))
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut body = json!({
        "model": model,
        "max_tokens": 4096,
        "messages": request.messages.iter().filter(|message| !matches!(message.role, StreamRole::System)).map(message_body).collect::<Vec<_>>(),
        "stream": true,
    });
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    if !request.tools.is_empty() {
        body["tools"] = json!(request.tools.iter().map(message_tool).collect::<Vec<_>>());
    }
    body
}

fn message_body(message: &StreamMessage) -> serde_json::Value {
    let role = match message.role {
        StreamRole::Assistant => "assistant",
        StreamRole::Tool | StreamRole::User | StreamRole::System => "user",
    };
    json!({
        "role": role,
        "content": message.content,
    })
}

fn message_tool(tool: &StreamTool) -> serde_json::Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.input_schema,
    })
}

fn parse_event(data: &str) -> Result<Vec<StreamEvent>, LlmClientError> {
    Ok(anthropic::stream_events(data)
        .map_err(|error| LlmClientError::Network(error.to_string()))?
        .into_iter()
        .map(|event| match event {
            AnthropicStreamEvent::AssistantTextDelta(delta) => {
                StreamEvent::AssistantTextDelta(delta)
            }
            AnthropicStreamEvent::ToolUseStarted { index, name } => {
                let id = tool_call_id(index);
                StreamEvent::ToolCallStarted { id, index, name }
            }
            AnthropicStreamEvent::ToolUseInputDelta { index, delta } => {
                let id = tool_call_id(index);
                StreamEvent::ToolInputDelta { id, index, delta }
            }
            AnthropicStreamEvent::ContentBlockStopped { index } => {
                let id = tool_call_id(index);
                StreamEvent::ToolCallCompleted { id, index }
            }
            AnthropicStreamEvent::TurnCompleted => StreamEvent::TurnCompleted {
                stop_reason: StopReason::EndTurn,
                provider_session_id: None,
            },
        })
        .collect())
}

fn tool_call_id(index: u64) -> String {
    format!("tool-call-{index}")
}

fn trim_base_url(base_url: String) -> String {
    base_url.trim_end_matches('/').to_string()
}
