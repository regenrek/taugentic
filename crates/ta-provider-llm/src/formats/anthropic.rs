use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u64,
    pub messages: Vec<Message>,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

pub fn request(model: &str, objective: &str) -> MessagesRequest {
    MessagesRequest {
        model: model.to_string(),
        max_tokens: 4096,
        messages: vec![Message {
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
    display_name: Option<String>,
}

pub fn models_response(data: &str) -> Result<Vec<ModelSummary>, serde_json::Error> {
    let response = serde_json::from_str::<ModelsResponse>(data)?;
    Ok(response
        .data
        .into_iter()
        .filter(|model| !model.id.trim().is_empty())
        .map(|model| ModelSummary {
            display_name: model
                .display_name
                .filter(|display_name| !display_name.trim().is_empty())
                .unwrap_or_else(|| model.id.clone()),
            id: model.id,
        })
        .collect())
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicWireEvent {
    ContentBlockStart {
        index: u64,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: u64,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: u64,
    },
    MessageDelta {},
    MessageStop {},
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentBlock {
    Text {},
    ToolUse { name: String },
    Thinking {},
    RedactedThinking {},
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentDelta {
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(rename = "input_json_delta")]
    InputJson { partial_json: String },
    #[serde(rename = "thinking_delta")]
    Thinking {},
    #[serde(rename = "signature_delta")]
    Signature {},
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnthropicStreamEvent {
    AssistantTextDelta(String),
    ToolUseStarted { index: u64, name: String },
    ToolUseInputDelta { index: u64, delta: String },
    ContentBlockStopped { index: u64 },
    TurnCompleted,
}

pub fn stream_events(data: &str) -> Result<Vec<AnthropicStreamEvent>, serde_json::Error> {
    let event = serde_json::from_str::<AnthropicWireEvent>(data)?;
    let events = match event {
        AnthropicWireEvent::ContentBlockStart {
            index,
            content_block: ContentBlock::ToolUse { name },
        } => vec![AnthropicStreamEvent::ToolUseStarted { index, name }],
        AnthropicWireEvent::ContentBlockDelta {
            delta: ContentDelta::Text { text },
            ..
        } if !text.is_empty() => vec![AnthropicStreamEvent::AssistantTextDelta(text)],
        AnthropicWireEvent::ContentBlockDelta {
            index,
            delta: ContentDelta::InputJson { partial_json },
        } if !partial_json.is_empty() => vec![AnthropicStreamEvent::ToolUseInputDelta {
            index,
            delta: partial_json,
        }],
        AnthropicWireEvent::ContentBlockStop { index } => {
            vec![AnthropicStreamEvent::ContentBlockStopped { index }]
        }
        AnthropicWireEvent::MessageDelta { .. } => Vec::new(),
        AnthropicWireEvent::MessageStop {} => vec![AnthropicStreamEvent::TurnCompleted],
        _ => Vec::new(),
    };
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_delta_stop_reason_does_not_complete_turn() {
        let events =
            stream_events("{\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}")
                .expect("message_delta parses");

        assert!(events.is_empty());
    }

    #[test]
    fn message_stop_completes_turn_once() {
        let events = stream_events("{\"type\":\"message_stop\"}").expect("message_stop parses");

        assert_eq!(events, vec![AnthropicStreamEvent::TurnCompleted]);
    }
}
