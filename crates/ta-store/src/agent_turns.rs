use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ta_protocol::wire::{
    ActivityCursor, AgentAssistantRow, AgentPendingStateRow, AgentStreamEvent, AgentStreamFrame,
    AgentStreamItemId, AgentStreamTurnId, AgentToolCallRow, AgentTurnRow, RunId, SessionId,
};

use crate::{EventRecord, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAgentTurnsPageQuery {
    pub session_id: SessionId,
    pub before_sequence: Option<u64>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAgentTurnsPage {
    pub rows: Vec<AgentTurnRow>,
    pub next_before_sequence: Option<u64>,
    pub latest_activity_sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AssistantTurnKey {
    pub session_id: SessionId,
    pub run_id: RunId,
    pub turn_id: Option<AgentStreamTurnId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ToolCallKey {
    pub session_id: SessionId,
    pub run_id: RunId,
    pub turn_id: Option<AgentStreamTurnId>,
    pub item_id: Option<AgentStreamItemId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InFlightAssistantTurn {
    pub session_id: SessionId,
    pub run_id: RunId,
    pub turn_id: Option<AgentStreamTurnId>,
    pub started_at_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InFlightToolCall {
    pub session_id: SessionId,
    pub run_id: RunId,
    pub turn_id: Option<AgentStreamTurnId>,
    pub item_id: Option<AgentStreamItemId>,
    pub tool_name: String,
    pub input: String,
    pub started_at_ms: u64,
    pub output: String,
}

pub fn row_sequence(row: &AgentTurnRow) -> u64 {
    match row {
        AgentTurnRow::Assistant(row) => row.cursor.sequence,
        AgentTurnRow::ToolCall(row) => row.cursor.sequence,
        AgentTurnRow::PendingState(row) => row.cursor.sequence,
    }
}

pub fn row_session_id(row: &AgentTurnRow) -> &SessionId {
    match row {
        AgentTurnRow::Assistant(row) => &row.session_id,
        AgentTurnRow::ToolCall(row) => &row.session_id,
        AgentTurnRow::PendingState(row) => &row.session_id,
    }
}

pub fn apply_agent_stream_event(
    assistant_turns: &mut BTreeMap<AssistantTurnKey, InFlightAssistantTurn>,
    tool_calls: &mut BTreeMap<ToolCallKey, InFlightToolCall>,
    record: &EventRecord,
) -> Result<Option<AgentTurnRow>, StoreError> {
    let ta_protocol::wire::DaemonEvent::AgentStream(event) = &record.payload else {
        return Ok(None);
    };

    match &event.emission.frame {
        AgentStreamFrame::AssistantTurnStarted => {
            let key = assistant_turn_key(&record.session_id, event);
            if assistant_turns.contains_key(&key) {
                return Err(agent_turn_projection_error(
                    "assistant turn already started",
                    &record.session_id,
                    &event.run_id,
                    event.emission.turn_id.as_ref(),
                    None,
                ));
            }
            assistant_turns.insert(
                key,
                InFlightAssistantTurn {
                    session_id: record.session_id.clone(),
                    run_id: event.run_id.clone(),
                    turn_id: event.emission.turn_id.clone(),
                    started_at_ms: record.occurred_at_ms,
                    text: String::new(),
                },
            );
            Ok(None)
        }
        AgentStreamFrame::AssistantMessageDelta { delta } => {
            if delta.is_empty() {
                return Ok(None);
            }
            let key = assistant_turn_key(&record.session_id, event);
            let state = assistant_turns.get_mut(&key).ok_or_else(|| {
                agent_turn_projection_error(
                    "assistant delta without started turn",
                    &record.session_id,
                    &event.run_id,
                    event.emission.turn_id.as_ref(),
                    None,
                )
            })?;
            state.text.push_str(delta);
            Ok(None)
        }
        AgentStreamFrame::AssistantTurnCompleted => {
            let key = assistant_turn_key(&record.session_id, event);
            let state = assistant_turns.remove(&key).ok_or_else(|| {
                agent_turn_projection_error(
                    "assistant completion without started turn",
                    &record.session_id,
                    &event.run_id,
                    event.emission.turn_id.as_ref(),
                    None,
                )
            })?;
            Ok(Some(AgentTurnRow::Assistant(AgentAssistantRow {
                cursor: ActivityCursor {
                    sequence: record.sequence,
                },
                session_id: state.session_id,
                run_id: state.run_id,
                turn_id: state.turn_id,
                started_at_ms: state.started_at_ms,
                completed_at_ms: record.occurred_at_ms,
                text: state.text,
            })))
        }
        AgentStreamFrame::ToolCallStarted { tool_name, input } => {
            let key = tool_call_key(&record.session_id, event);
            if tool_calls.contains_key(&key) {
                return Err(agent_turn_projection_error(
                    "tool call already started",
                    &record.session_id,
                    &event.run_id,
                    event.emission.turn_id.as_ref(),
                    event.emission.item_id.as_ref(),
                ));
            }
            tool_calls.insert(
                key,
                InFlightToolCall {
                    session_id: record.session_id.clone(),
                    run_id: event.run_id.clone(),
                    turn_id: event.emission.turn_id.clone(),
                    item_id: event.emission.item_id.clone(),
                    tool_name: tool_name.clone(),
                    input: input.clone(),
                    started_at_ms: record.occurred_at_ms,
                    output: String::new(),
                },
            );
            Ok(None)
        }
        AgentStreamFrame::ToolCallProgressed { delta } => {
            if delta.is_empty() {
                return Ok(None);
            }
            let key = tool_call_key(&record.session_id, event);
            let state = tool_calls.get_mut(&key).ok_or_else(|| {
                agent_turn_projection_error(
                    "tool progress without started call",
                    &record.session_id,
                    &event.run_id,
                    event.emission.turn_id.as_ref(),
                    event.emission.item_id.as_ref(),
                )
            })?;
            state.output.push_str(delta);
            Ok(None)
        }
        AgentStreamFrame::ToolCallCompleted { outcome } => {
            let key = tool_call_key(&record.session_id, event);
            let state = tool_calls.remove(&key).ok_or_else(|| {
                agent_turn_projection_error(
                    "tool completion without started call",
                    &record.session_id,
                    &event.run_id,
                    event.emission.turn_id.as_ref(),
                    event.emission.item_id.as_ref(),
                )
            })?;
            Ok(Some(AgentTurnRow::ToolCall(AgentToolCallRow {
                cursor: ActivityCursor {
                    sequence: record.sequence,
                },
                session_id: state.session_id,
                run_id: state.run_id,
                turn_id: state.turn_id,
                item_id: state.item_id,
                tool_name: state.tool_name,
                input: state.input,
                output: state.output,
                outcome: *outcome,
                started_at_ms: state.started_at_ms,
                completed_at_ms: record.occurred_at_ms,
            })))
        }
        AgentStreamFrame::PendingStateChanged { state } => {
            Ok(Some(AgentTurnRow::PendingState(AgentPendingStateRow {
                cursor: ActivityCursor {
                    sequence: record.sequence,
                },
                session_id: record.session_id.clone(),
                run_id: event.run_id.clone(),
                turn_id: event.emission.turn_id.clone(),
                occurred_at_ms: record.occurred_at_ms,
                state: *state,
            })))
        }
        AgentStreamFrame::TokenUsageUpdated { .. } => Ok(None),
    }
}

fn assistant_turn_key(session_id: &SessionId, event: &AgentStreamEvent) -> AssistantTurnKey {
    AssistantTurnKey {
        session_id: session_id.clone(),
        run_id: event.run_id.clone(),
        turn_id: event.emission.turn_id.clone(),
    }
}

fn tool_call_key(session_id: &SessionId, event: &AgentStreamEvent) -> ToolCallKey {
    ToolCallKey {
        session_id: session_id.clone(),
        run_id: event.run_id.clone(),
        turn_id: event.emission.turn_id.clone(),
        item_id: event.emission.item_id.clone(),
    }
}

fn agent_turn_projection_error(
    message: &str,
    session_id: &SessionId,
    run_id: &RunId,
    turn_id: Option<&AgentStreamTurnId>,
    item_id: Option<&AgentStreamItemId>,
) -> StoreError {
    let turn = turn_id.map(|value| value.as_str()).unwrap_or("__turn__");
    let item = item_id.map(|value| value.as_str()).unwrap_or("__item__");
    StoreError::AgentTurnProjectionViolation {
        detail: format!(
            "{message}: session={} run={} turn={} item={}",
            session_id.as_str(),
            run_id.as_str(),
            turn,
            item,
        ),
    }
}
