use serde::{Deserialize, Serialize};

use super::{ProviderStreamError, ProviderStreamEvent};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChatCompletionsRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub fn request(model: &str, objective: &str) -> ChatCompletionsRequest {
    ChatCompletionsRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: objective.to_string(),
        }],
        stream: true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSummary {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
}

pub fn models_response(data: &str) -> Result<Vec<ModelSummary>, serde_json::Error> {
    let response = serde_json::from_str::<ModelsResponse>(data)?;
    Ok(response
        .data
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| ModelSummary {
            display_name: model.id.clone(),
            id: model.id,
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    delta: ChatDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ChatDelta {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ToolCallDelta {
    index: u64,
    function: Option<ToolCallFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct ToolCallFunctionDelta {
    name: Option<String>,
    #[serde(default)]
    arguments: String,
}

pub fn stream_events(data: &str) -> Result<Vec<ProviderStreamEvent>, ProviderStreamError> {
    let chunk = serde_json::from_str::<ChatChunk>(data)?;
    let mut events = Vec::new();

    for choice in chunk.choices {
        if let Some(delta) = choice.delta.content.filter(|delta| !delta.is_empty()) {
            events.push(ProviderStreamEvent::AssistantTextDelta(delta));
        }

        for tool_call in choice.delta.tool_calls.unwrap_or_default() {
            if let Some(function) = tool_call.function {
                if let Some(name) = function.name.filter(|name| !name.is_empty()) {
                    events.push(ProviderStreamEvent::ToolCallStarted {
                        index: tool_call.index,
                        name,
                    });
                }
                if !function.arguments.is_empty() {
                    events.push(ProviderStreamEvent::ToolCallProgress {
                        index: tool_call.index,
                        delta: function.arguments,
                    });
                }
            }
        }

        match choice.finish_reason.as_deref() {
            Some("tool_calls") => events.push(ProviderStreamEvent::ToolCallBatchCompleted),
            Some(_) => events.push(ProviderStreamEvent::TurnCompleted),
            None => {}
        }
    }

    Ok(events)
}
