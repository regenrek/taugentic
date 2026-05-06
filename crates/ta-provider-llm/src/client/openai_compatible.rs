use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::{
    EventParser, HttpLlmStream, LlmClient, LlmStream, StreamEvent, StreamMessage, StreamRequest,
    StreamRole, StreamTool, map_provider_error, map_provider_events, model_or_default,
    require_stream_response, send_request,
};
use crate::error::LlmClientError;
use crate::families::openai_compatible::AuthSource;
use crate::formats::openai;
use crate::http::shared_client;
use tracing::instrument;

#[derive(Clone)]
pub struct OpenAiCompatibleClient {
    http: reqwest::Client,
    base_url: String,
    auth: AuthSource,
    model: String,
}

impl OpenAiCompatibleClient {
    pub fn new(
        base_url: impl Into<String>,
        auth: AuthSource,
        model: impl Into<String>,
    ) -> Result<Self, LlmClientError> {
        let base_url = base_url.into();
        if base_url.trim().is_empty() {
            return Err(LlmClientError::InvalidConfig(
                "OpenAI-compatible base URL is empty".to_string(),
            ));
        }
        let model = model.into();
        if model.trim().is_empty() {
            return Err(LlmClientError::InvalidConfig(
                "OpenAI-compatible model is empty".to_string(),
            ));
        }
        Ok(Self {
            http: shared_client(),
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
            model,
        })
    }

    pub fn with_bearer(
        base_url: impl Into<String>,
        token: impl Into<Arc<str>>,
        model: impl Into<String>,
    ) -> Result<Self, LlmClientError> {
        Self::new(base_url, AuthSource::BearerStatic(token.into()), model)
    }

    async fn send(
        &self,
        request: StreamRequest,
        cancellation: &CancellationToken,
    ) -> Result<reqwest::Response, LlmClientError> {
        let body = serde_json::to_vec(&chat_body(&request, &self.model))
            .map_err(|error| LlmClientError::InvalidConfig(error.to_string()))?;
        let mut builder = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body);

        match &self.auth {
            AuthSource::BearerEnv(var) => {
                let token = std::env::var(var)
                    .map_err(|_| LlmClientError::CredentialsMissing(format!("{var} is not set")))?;
                builder = builder.bearer_auth(token);
            }
            AuthSource::BearerStatic(token) => {
                builder = builder.bearer_auth(token.as_ref());
            }
        }

        let response = send_request(builder, cancellation).await?;
        require_stream_response(response, cancellation).await
    }
}

#[async_trait]
impl LlmClient for OpenAiCompatibleClient {
    #[instrument(level = "debug", skip_all, fields(provider = "openai_compatible"))]
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

fn chat_body(request: &StreamRequest, default_model: &str) -> serde_json::Value {
    let mut body = json!({
        "model": model_or_default(request, default_model),
        "messages": request.messages.iter().map(chat_message).collect::<Vec<_>>(),
        "stream": true,
    });
    if !request.tools.is_empty() {
        body["tools"] = json!(request.tools.iter().map(chat_tool).collect::<Vec<_>>());
    }
    body
}

fn chat_message(message: &StreamMessage) -> serde_json::Value {
    let role = match message.role {
        StreamRole::System => "system",
        StreamRole::User => "user",
        StreamRole::Assistant => "assistant",
        StreamRole::Tool => "tool",
    };
    let mut value = json!({
        "role": role,
        "content": message.content,
    });
    if let Some(id) = message.tool_call_id.as_deref() {
        value["tool_call_id"] = json!(id);
    }
    value
}

fn chat_tool(tool: &StreamTool) -> serde_json::Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema,
        }
    })
}

fn parse_event(data: &str) -> Result<Vec<StreamEvent>, LlmClientError> {
    openai::stream_events(data)
        .map(map_provider_events)
        .map_err(map_provider_error)
}
