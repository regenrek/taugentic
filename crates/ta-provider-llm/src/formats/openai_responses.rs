use serde::{Deserialize, Serialize};

use super::{ProviderStreamError, ProviderStreamEvent, ProviderStreamFailure, ProviderTokenUsage};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResponsesRequest {
    pub model: String,
    pub input: Vec<ResponsesInputMessage>,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResponsesInputMessage {
    pub role: String,
    pub content: Vec<ResponsesInputContent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponsesInputContent {
    InputText { text: String },
}

pub fn request(model: &str, objective: &str) -> ResponsesRequest {
    ResponsesRequest {
        model: model.to_string(),
        input: vec![ResponsesInputMessage {
            role: "user".to_string(),
            content: vec![ResponsesInputContent::InputText {
                text: objective.to_string(),
            }],
        }],
        stream: true,
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ResponsesStreamEvent {
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta { delta: String },
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded { output_index: u64, item: OutputItem },
    #[serde(rename = "response.function_call_arguments.delta")]
    FunctionCallArgumentsDelta { output_index: u64, delta: String },
    #[serde(rename = "response.output_item.done")]
    OutputItemDone { output_index: u64, item: OutputItem },
    #[serde(rename = "response.completed")]
    ResponseCompleted {
        response: Option<ResponseCompletedEnvelope>,
    },
    #[serde(rename = "response.failed")]
    ResponseFailed {
        response: Option<ResponseFailureEnvelope>,
        error: Option<ResponseError>,
    },
    #[serde(rename = "response.incomplete")]
    ResponseIncomplete {
        response: Option<ResponseIncompleteEnvelope>,
    },
    #[serde(rename = "error")]
    Error { error: ResponseError },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct ResponseFailureEnvelope {
    error: Option<ResponseError>,
}

#[derive(Debug, Deserialize)]
struct ResponseIncompleteEnvelope {
    incomplete_details: Option<ResponseIncompleteDetails>,
}

#[derive(Debug, Deserialize)]
struct ResponseIncompleteDetails {
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseCompletedEnvelope {
    model: Option<String>,
    usage: Option<ResponseUsage>,
}

#[derive(Debug, Deserialize)]
struct ResponseUsage {
    input_tokens: u64,
    output_tokens: u64,
    input_tokens_details: Option<ResponseInputTokenDetails>,
    output_tokens_details: Option<ResponseOutputTokenDetails>,
}

#[derive(Debug, Deserialize)]
struct ResponseInputTokenDetails {
    cached_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ResponseOutputTokenDetails {
    reasoning_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ResponseError {
    code: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputItem {
    FunctionCall { name: String },
    Message {},
    Reasoning {},
}

pub fn stream_events(data: &str) -> Result<Vec<ProviderStreamEvent>, ProviderStreamError> {
    let event = serde_json::from_str::<ResponsesStreamEvent>(data)?;
    let events = match event {
        ResponsesStreamEvent::OutputTextDelta { delta } if !delta.is_empty() => {
            vec![ProviderStreamEvent::AssistantTextDelta(delta)]
        }
        ResponsesStreamEvent::OutputItemAdded {
            output_index,
            item: OutputItem::FunctionCall { name },
        } => vec![ProviderStreamEvent::ToolCallStarted {
            index: output_index,
            name,
        }],
        ResponsesStreamEvent::FunctionCallArgumentsDelta {
            output_index,
            delta,
        } if !delta.is_empty() => vec![ProviderStreamEvent::ToolCallProgress {
            index: output_index,
            delta,
        }],
        ResponsesStreamEvent::OutputItemDone {
            output_index,
            item: OutputItem::FunctionCall { .. },
        } => vec![ProviderStreamEvent::ToolCallCompleted {
            index: output_index,
        }],
        ResponsesStreamEvent::ResponseFailed { response, error } => {
            return Err(ProviderStreamError::Failure(failure_from_response_error(
                "response.failed",
                response.and_then(|response| response.error).or(error),
            )));
        }
        ResponsesStreamEvent::ResponseIncomplete { response } => {
            let reason = response
                .and_then(|response| response.incomplete_details)
                .and_then(|details| details.reason)
                .unwrap_or_else(|| "unknown".to_string());
            return Err(ProviderStreamError::Failure(ProviderStreamFailure {
                code: Some("response_incomplete".to_string()),
                message: format!("response.incomplete: {reason}"),
            }));
        }
        ResponsesStreamEvent::Error { error } => {
            return Err(ProviderStreamError::Failure(failure_from_response_error(
                "error",
                Some(error),
            )));
        }
        ResponsesStreamEvent::ResponseCompleted { response } => {
            let mut events = Vec::new();
            if let Some(usage) = response.and_then(token_usage_from_completed_response) {
                events.push(ProviderStreamEvent::TokenUsage(usage));
            }
            events.push(ProviderStreamEvent::TurnCompleted);
            events
        }
        _ => Vec::new(),
    };
    Ok(events)
}

fn token_usage_from_completed_response(
    response: ResponseCompletedEnvelope,
) -> Option<ProviderTokenUsage> {
    let usage = response.usage?;
    Some(ProviderTokenUsage {
        prompt_tokens: usage.input_tokens,
        completion_tokens: usage.output_tokens,
        cached_tokens: usage
            .input_tokens_details
            .and_then(|details| details.cached_tokens),
        reasoning_tokens: usage
            .output_tokens_details
            .and_then(|details| details.reasoning_tokens),
        model: response.model,
    })
}

fn failure_from_response_error(
    fallback: &str,
    error: Option<ResponseError>,
) -> ProviderStreamFailure {
    let Some(error) = error else {
        return ProviderStreamFailure {
            code: Some(fallback.to_string()),
            message: format!("{fallback} event received"),
        };
    };
    ProviderStreamFailure {
        code: error.code,
        message: error
            .message
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| format!("{fallback} event received")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_completed_with_response_payload_finishes_turn() {
        let events = stream_events(
            "{\"type\":\"response.completed\",\"response\":{\"id\":\"resp_platform\"}}",
        )
        .expect("response.completed should parse");

        assert_eq!(events, vec![ProviderStreamEvent::TurnCompleted]);
    }

    #[test]
    fn response_completed_with_usage_records_token_usage_before_turn_completion() {
        let events = stream_events(
            "{\"type\":\"response.completed\",\"response\":{\"id\":\"resp_platform\",\"model\":\"gpt-test\",\"usage\":{\"input_tokens\":11,\"output_tokens\":7,\"input_tokens_details\":{\"cached_tokens\":3},\"output_tokens_details\":{\"reasoning_tokens\":2}}}}",
        )
        .expect("response.completed should parse");

        assert_eq!(
            events,
            vec![
                ProviderStreamEvent::TokenUsage(ProviderTokenUsage {
                    prompt_tokens: 11,
                    completion_tokens: 7,
                    cached_tokens: Some(3),
                    reasoning_tokens: Some(2),
                    model: Some("gpt-test".to_string()),
                }),
                ProviderStreamEvent::TurnCompleted,
            ]
        );
    }

    #[test]
    fn response_completed_missing_usage_fields_keeps_completion_without_usage() {
        let events = stream_events(
            "{\"type\":\"response.completed\",\"response\":{\"id\":\"resp_platform\"}}",
        )
        .expect("response.completed should parse");

        assert_eq!(events, vec![ProviderStreamEvent::TurnCompleted]);
    }

    #[test]
    fn response_failed_becomes_stream_failure() {
        let error = stream_events(
            "{\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"context_length_exceeded\",\"message\":\"too long\"}}}",
        )
        .expect_err("response.failed must fail");

        assert!(
            matches!(
                error,
                ProviderStreamError::Failure(ProviderStreamFailure {
                    code: Some(ref code),
                    ref message,
                }) if code == "context_length_exceeded" && message == "too long"
            ),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn response_incomplete_becomes_stream_failure() {
        let error = stream_events(
            "{\"type\":\"response.incomplete\",\"response\":{\"incomplete_details\":{\"reason\":\"max_output_tokens\"}}}",
        )
        .expect_err("response.incomplete must fail");

        assert!(
            matches!(
                error,
                ProviderStreamError::Failure(ProviderStreamFailure {
                    code: Some(ref code),
                    ref message,
                }) if code == "response_incomplete" && message.contains("max_output_tokens")
            ),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn top_level_error_becomes_stream_failure() {
        let error = stream_events(
            "{\"type\":\"error\",\"error\":{\"code\":\"rate_limit_exceeded\",\"message\":\"slow down\"}}",
        )
        .expect_err("error event must fail");

        assert!(
            matches!(
                error,
                ProviderStreamError::Failure(ProviderStreamFailure {
                    code: Some(ref code),
                    ref message,
                }) if code == "rate_limit_exceeded" && message == "slow down"
            ),
            "unexpected error: {error:?}"
        );
    }
}
