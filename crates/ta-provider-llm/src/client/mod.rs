pub mod anthropic_messages;
pub mod openai_compatible;
pub mod openai_responses;

use std::collections::VecDeque;
use std::future::Future;

use async_trait::async_trait;
use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use tracing::instrument;

use crate::error::LlmClientError;
use crate::formats::{ProviderStreamError, ProviderStreamEvent};

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn start_stream(
        &self,
        request: StreamRequest,
        cancellation: CancellationToken,
    ) -> Result<Box<dyn LlmStream>, LlmClientError>;

    fn supports_parallel_tool_calls(&self) -> bool;
}

pub trait LlmStream: Send + Unpin {
    fn next_event(&mut self) -> BoxFuture<'_, Result<Option<StreamEvent>, LlmClientError>>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamRequest {
    pub model: String,
    pub messages: Vec<StreamMessage>,
    pub tools: Vec<StreamTool>,
    pub provider_session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamMessage {
    pub role: StreamRole,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<StreamToolCallRecord>,
}

impl StreamMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: StreamRole::System,
            content: content.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: StreamRole::User,
            content: content.into(),
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    pub fn assistant(content: impl Into<String>, tool_calls: Vec<StreamToolCallRecord>) -> Self {
        Self {
            role: StreamRole::Assistant,
            content: content.into(),
            tool_call_id: None,
            tool_calls,
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: StreamRole::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamToolCallRecord {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    AssistantTextDelta(String),
    ToolCallStarted {
        id: String,
        index: u64,
        name: String,
    },
    ToolInputDelta {
        id: String,
        index: u64,
        delta: String,
    },
    ToolCallCompleted {
        id: String,
        index: u64,
    },
    ToolCallBatchCompleted,
    TokenUsage(LlmTokenUsage),
    TurnCompleted {
        stop_reason: StopReason,
        provider_session_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmTokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub model: String,
    pub provider: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    ToolCalls,
    MaxTokens,
    Unknown(String),
}

pub struct VecLlmStream {
    events: VecDeque<Result<StreamEvent, LlmClientError>>,
}

impl VecLlmStream {
    pub fn new(events: Vec<Result<StreamEvent, LlmClientError>>) -> Self {
        Self {
            events: VecDeque::from(events),
        }
    }
}

impl LlmStream for VecLlmStream {
    fn next_event(&mut self) -> BoxFuture<'_, Result<Option<StreamEvent>, LlmClientError>> {
        Box::pin(async move {
            match self.events.pop_front() {
                Some(Ok(event)) => Ok(Some(event)),
                Some(Err(error)) => Err(error),
                None => Ok(None),
            }
        })
    }
}

pub(crate) fn map_failure(code: Option<&str>, message: String) -> LlmClientError {
    match code.unwrap_or_default() {
        "context_length_exceeded" | "context_window_exceeded" => {
            LlmClientError::ContextLengthExceeded(message)
        }
        "rate_limit_exceeded" => LlmClientError::RateLimited {
            retry_after_ms: None,
            detail: message,
        },
        _ => LlmClientError::ServerError(message),
    }
}

pub(crate) fn model_or_default(request: &StreamRequest, default_model: &str) -> String {
    if request.model.trim().is_empty() {
        default_model.to_string()
    } else {
        request.model.clone()
    }
}

pub(crate) type EventParser = fn(&str) -> Result<Vec<StreamEvent>, LlmClientError>;

pub(crate) struct HttpLlmStream {
    body: BoxStream<'static, Result<Vec<u8>, LlmClientError>>,
    buffered_events: VecDeque<StreamEvent>,
    buffered_bytes: Vec<u8>,
    parser: EventParser,
    cancellation: CancellationToken,
    require_turn_completed: bool,
    saw_turn_completed: bool,
}

impl HttpLlmStream {
    pub(crate) fn new(
        response: reqwest::Response,
        parser: EventParser,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            body: response
                .bytes_stream()
                .map(|chunk| {
                    chunk
                        .map(|bytes| bytes.to_vec())
                        .map_err(|error| LlmClientError::Network(error.to_string()))
                })
                .boxed(),
            buffered_events: VecDeque::new(),
            buffered_bytes: Vec::new(),
            parser,
            cancellation,
            require_turn_completed: false,
            saw_turn_completed: false,
        }
    }

    pub(crate) fn new_requiring_turn_completed(
        response: reqwest::Response,
        parser: EventParser,
        cancellation: CancellationToken,
    ) -> Self {
        let mut stream = Self::new(response, parser, cancellation);
        stream.require_turn_completed = true;
        stream
    }

    #[cfg(test)]
    fn from_body(
        body: impl futures_util::Stream<Item = Result<Vec<u8>, LlmClientError>> + Send + 'static,
        parser: EventParser,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            body: body.boxed(),
            buffered_events: VecDeque::new(),
            buffered_bytes: Vec::new(),
            parser,
            cancellation,
            require_turn_completed: false,
            saw_turn_completed: false,
        }
    }

    #[instrument(level = "trace", skip_all)]
    async fn read_next(&mut self) -> Result<Option<StreamEvent>, LlmClientError> {
        loop {
            if let Some(event) = self.buffered_events.pop_front() {
                return Ok(Some(self.mark_terminal_event(event)));
            }
            while let Some((frame_end, delimiter_len)) = sse_frame_end(&self.buffered_bytes) {
                let frame_bytes = self.buffered_bytes[..frame_end].to_vec();
                self.buffered_bytes.drain(..frame_end + delimiter_len);
                let frame = String::from_utf8(frame_bytes)
                    .map_err(|error| LlmClientError::Network(error.to_string()))?;
                let data = sse_data(&frame);
                if data.is_empty() || data == "[DONE]" {
                    continue;
                }
                self.buffered_events.extend((self.parser)(&data)?);
                if let Some(event) = self.buffered_events.pop_front() {
                    return Ok(Some(self.mark_terminal_event(event)));
                }
            }
            let maybe_chunk = tokio::select! {
                biased;
                _ = self.cancellation.cancelled() => {
                    return Err(LlmClientError::Cancelled("stream cancelled".to_string()));
                }
                chunk = self.body.next() => chunk,
            };
            let Some(chunk) = maybe_chunk else {
                if self.require_turn_completed && !self.saw_turn_completed {
                    return Err(LlmClientError::Network(
                        "stream closed before response.completed".to_string(),
                    ));
                }
                return Ok(None);
            };
            let chunk = chunk?;
            self.buffered_bytes.extend(chunk);
        }
    }

    fn mark_terminal_event(&mut self, event: StreamEvent) -> StreamEvent {
        if matches!(event, StreamEvent::TurnCompleted { .. }) {
            self.saw_turn_completed = true;
        }
        event
    }
}

impl LlmStream for HttpLlmStream {
    fn next_event(&mut self) -> BoxFuture<'_, Result<Option<StreamEvent>, LlmClientError>> {
        Box::pin(async move { self.read_next().await })
    }
}

#[instrument(level = "trace", skip_all)]
pub(crate) async fn send_request(
    request: reqwest::RequestBuilder,
    cancellation: &CancellationToken,
) -> Result<reqwest::Response, LlmClientError> {
    cancellable(
        cancellation,
        "stream cancelled during request",
        async move {
            request
                .send()
                .await
                .map_err(|error| LlmClientError::Network(error.to_string()))
        },
    )
    .await
}

async fn cancellable<T>(
    cancellation: &CancellationToken,
    message: &'static str,
    future: impl Future<Output = Result<T, LlmClientError>>,
) -> Result<T, LlmClientError> {
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(LlmClientError::Cancelled(message.to_string())),
        result = future => result,
    }
}

#[instrument(level = "trace", skip_all)]
pub(crate) async fn require_stream_response(
    response: reqwest::Response,
    cancellation: &CancellationToken,
) -> Result<reqwest::Response, LlmClientError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = tokio::select! {
        biased;
        _ = cancellation.cancelled() => {
            return Err(LlmClientError::Cancelled("stream cancelled while reading error response".to_string()));
        }
        body = response.text() => body.unwrap_or_default(),
    };
    let detail = if body.trim().is_empty() {
        status.to_string()
    } else {
        body
    };

    match status.as_u16() {
        401 | 403 => Err(LlmClientError::Auth(detail)),
        429 => Err(LlmClientError::RateLimited {
            retry_after_ms: None,
            detail,
        }),
        400 if detail.to_ascii_lowercase().contains("context") => {
            Err(LlmClientError::ContextLengthExceeded(detail))
        }
        400 => Err(LlmClientError::InvalidConfig(detail)),
        500..=599 => Err(LlmClientError::ServerError(detail)),
        _ => Err(LlmClientError::Network(detail)),
    }
}

pub(crate) fn map_provider_events(events: Vec<ProviderStreamEvent>) -> Vec<StreamEvent> {
    map_provider_events_with_metadata(events, "unknown", "")
}

pub(crate) fn map_provider_events_with_metadata(
    events: Vec<ProviderStreamEvent>,
    provider: &str,
    fallback_model: &str,
) -> Vec<StreamEvent> {
    events
        .into_iter()
        .map(|event| match event {
            ProviderStreamEvent::AssistantTextDelta(delta) => {
                StreamEvent::AssistantTextDelta(delta)
            }
            ProviderStreamEvent::ToolCallStarted { index, name } => {
                let id = tool_call_id(index);
                StreamEvent::ToolCallStarted { id, index, name }
            }
            ProviderStreamEvent::ToolCallProgress { index, delta } => {
                let id = tool_call_id(index);
                StreamEvent::ToolInputDelta { id, index, delta }
            }
            ProviderStreamEvent::ToolCallCompleted { index } => {
                let id = tool_call_id(index);
                StreamEvent::ToolCallCompleted { id, index }
            }
            ProviderStreamEvent::ToolCallBatchCompleted => StreamEvent::ToolCallBatchCompleted,
            ProviderStreamEvent::TokenUsage(usage) => StreamEvent::TokenUsage(LlmTokenUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                cached_tokens: usage.cached_tokens,
                reasoning_tokens: usage.reasoning_tokens,
                model: usage
                    .model
                    .filter(|model| !model.trim().is_empty())
                    .unwrap_or_else(|| fallback_model.to_string()),
                provider: provider.to_string(),
            }),
            ProviderStreamEvent::TurnCompleted => StreamEvent::TurnCompleted {
                stop_reason: StopReason::EndTurn,
                provider_session_id: None,
            },
        })
        .collect()
}

pub(crate) fn map_provider_error(error: ProviderStreamError) -> LlmClientError {
    match error {
        ProviderStreamError::Json(error) => LlmClientError::Network(error.to_string()),
        ProviderStreamError::Failure(failure) => {
            map_failure(failure.code.as_deref(), failure.message)
        }
    }
}

fn sse_data(frame: &str) -> String {
    frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n")
}

fn sse_frame_end(bytes: &[u8]) -> Option<(usize, usize)> {
    let lf = bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2));
    let crlf = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| (index, 4));
    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn tool_call_id(index: u64) -> String {
    format!("tool-call-{index}")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::stream;

    use super::*;

    fn echo_parser(data: &str) -> Result<Vec<StreamEvent>, LlmClientError> {
        Ok(vec![StreamEvent::AssistantTextDelta(data.to_string())])
    }

    fn terminal_parser(data: &str) -> Result<Vec<StreamEvent>, LlmClientError> {
        Ok(match data {
            "done" => vec![StreamEvent::TurnCompleted {
                stop_reason: StopReason::EndTurn,
                provider_session_id: None,
            }],
            value => vec![StreamEvent::AssistantTextDelta(value.to_string())],
        })
    }

    #[tokio::test]
    async fn sse_parser_preserves_utf8_split_across_chunks() {
        let bytes = "data: café\n\n".as_bytes();
        let split = bytes
            .windows(2)
            .position(|window| window == "é".as_bytes())
            .expect("accent byte");
        let body = stream::iter(vec![
            Ok(bytes[..split + 1].to_vec()),
            Ok(bytes[split + 1..].to_vec()),
        ]);
        let mut stream = HttpLlmStream::from_body(body, echo_parser, CancellationToken::new());

        let event = stream.next_event().await.expect("event");

        assert_eq!(
            event,
            Some(StreamEvent::AssistantTextDelta("café".to_string()))
        );
    }

    #[tokio::test]
    async fn sse_parser_drains_multiple_frames_from_one_chunk() {
        let body = stream::iter(vec![Ok(b"data: pong\n\ndata: done\n\n".to_vec())]);
        let mut stream = HttpLlmStream::from_body(body, terminal_parser, CancellationToken::new());

        assert_eq!(
            stream.next_event().await.expect("text event"),
            Some(StreamEvent::AssistantTextDelta("pong".to_string()))
        );
        assert_eq!(
            stream.next_event().await.expect("terminal event"),
            Some(StreamEvent::TurnCompleted {
                stop_reason: StopReason::EndTurn,
                provider_session_id: None,
            })
        );
        assert_eq!(stream.next_event().await.expect("eof"), None);
    }

    #[tokio::test]
    async fn sse_stream_unblocks_when_cancelled_while_waiting_for_chunk() {
        let cancellation = CancellationToken::new();
        let mut stream = HttpLlmStream::from_body(
            stream::pending::<Result<Vec<u8>, LlmClientError>>(),
            echo_parser,
            cancellation.clone(),
        );
        let task = tokio::spawn(async move { stream.next_event().await });

        tokio::task::yield_now().await;
        cancellation.cancel();
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("cancelled stream should not hang")
            .expect("task should not panic");

        assert!(matches!(result, Err(LlmClientError::Cancelled(_))));
    }

    #[tokio::test]
    async fn send_request_unblocks_when_cancelled_while_waiting_for_headers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("local address");
        let server = tokio::spawn(async move {
            let _connection = listener.accept().await.expect("accept");
            futures_util::future::pending::<()>().await;
        });
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            let client = reqwest::Client::new();
            send_request(
                client.get(format!("http://{address}/stream")),
                &task_cancellation,
            )
            .await
        });

        tokio::task::yield_now().await;
        cancellation.cancel();
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("cancelled request should not hang")
            .expect("task should not panic");
        server.abort();

        assert!(matches!(result, Err(LlmClientError::Cancelled(_))));
    }
}
