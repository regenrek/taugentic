use serde_json::Value;
use ta_protocol::wire::{
    AgentStreamFrame, AgentStreamItemId, AgentStreamTurnId, AgentToolCallOutcome, StreamEmission,
};

use super::string_field;
use crate::error::AcpClientError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpClientEvent {
    AssistantTextDelta(String),
    ToolCallStarted {
        id: Option<String>,
        tool_name: String,
    },
    ToolCallProgress {
        id: Option<String>,
        delta: String,
    },
    ToolCallCompleted {
        id: Option<String>,
        outcome: AgentToolCallOutcome,
    },
}

pub(super) struct AcpStreamEmissionMapper {
    turn_id: AgentStreamTurnId,
    fragment_sequence: u64,
}

impl AcpStreamEmissionMapper {
    pub(super) fn new(turn_id: AgentStreamTurnId) -> Self {
        Self {
            turn_id,
            fragment_sequence: 0,
        }
    }

    pub(super) fn lifecycle(&mut self, frame: AgentStreamFrame) -> StreamEmission {
        self.fragment_sequence += 1;
        StreamEmission {
            turn_id: Some(self.turn_id.clone()),
            item_id: None,
            fragment_sequence: Some(self.fragment_sequence),
            frame,
        }
    }

    pub(super) fn map(&mut self, event: AcpClientEvent) -> Result<StreamEmission, AcpClientError> {
        let (frame, item_id) = match event {
            AcpClientEvent::AssistantTextDelta(delta) => {
                (AgentStreamFrame::AssistantMessageDelta { delta }, None)
            }
            AcpClientEvent::ToolCallStarted { id, tool_name } => (
                AgentStreamFrame::ToolCallStarted {
                    tool_name,
                    input: "null".to_string(),
                },
                id,
            ),
            AcpClientEvent::ToolCallProgress { id, delta } => {
                (AgentStreamFrame::ToolCallProgressed { delta }, id)
            }
            AcpClientEvent::ToolCallCompleted { id, outcome } => {
                (AgentStreamFrame::ToolCallCompleted { outcome }, id)
            }
        };
        self.fragment_sequence += 1;
        Ok(StreamEmission {
            turn_id: Some(self.turn_id.clone()),
            item_id: item_id.map(item_id_from_string).transpose()?,
            fragment_sequence: Some(self.fragment_sequence),
            frame,
        })
    }
}

pub(super) fn turn_id(id: &str) -> Result<AgentStreamTurnId, AcpClientError> {
    AgentStreamTurnId::new(id).map_err(|error| AcpClientError::ProcessFailed(error.to_string()))
}

fn item_id_from_string(id: String) -> Result<AgentStreamItemId, AcpClientError> {
    AgentStreamItemId::new(id).map_err(|error| AcpClientError::ProcessFailed(error.to_string()))
}

pub(super) fn session_update_events(params: Option<Value>) -> Vec<AcpClientEvent> {
    let Some(update) = params.and_then(|params| params.get("update").cloned()) else {
        return Vec::new();
    };
    match update.get("sessionUpdate").and_then(Value::as_str) {
        Some("agent_message_chunk" | "agent_thought_chunk") => text_from_update(&update)
            .map(AcpClientEvent::AssistantTextDelta)
            .into_iter()
            .collect(),
        Some("tool_call") => {
            let id = string_field(&update, "toolCallId").or_else(|| string_field(&update, "id"));
            let tool_name = string_field(&update, "title")
                .or_else(|| string_field(&update, "kind"))
                .unwrap_or_else(|| "tool".to_string());
            vec![AcpClientEvent::ToolCallStarted { id, tool_name }]
        }
        Some("tool_call_update") => {
            let id = string_field(&update, "toolCallId").or_else(|| string_field(&update, "id"));
            let mut events = Vec::new();
            if let Some(delta) = tool_progress_delta(&update) {
                events.push(AcpClientEvent::ToolCallProgress {
                    id: id.clone(),
                    delta,
                });
            }
            if let Some(outcome) = tool_outcome(&update) {
                events.push(AcpClientEvent::ToolCallCompleted { id, outcome });
            }
            events
        }
        _ => Vec::new(),
    }
}

fn text_from_update(update: &Value) -> Option<String> {
    update
        .pointer("/content/content/text")
        .or_else(|| update.pointer("/content/text"))
        .or_else(|| update.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn tool_progress_delta(update: &Value) -> Option<String> {
    update
        .pointer("/fields/content/0/content/text")
        .or_else(|| update.pointer("/fields/rawOutput"))
        .or_else(|| update.pointer("/rawOutput"))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| Some(value.to_string()))
        })
}

fn tool_outcome(update: &Value) -> Option<AgentToolCallOutcome> {
    let status = update
        .pointer("/fields/status")
        .or_else(|| update.get("status"))
        .and_then(Value::as_str)?;
    match status {
        "completed" => Some(AgentToolCallOutcome::Completed),
        "failed" => Some(AgentToolCallOutcome::Failed),
        "cancelled" => Some(AgentToolCallOutcome::Cancelled),
        _ => None,
    }
}
