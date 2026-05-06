use std::collections::BTreeSet;

use ta_protocol::wire::{
    AgentStreamFrame, AgentStreamTurnId, AgentToolCallRow, AgentTurnRow, RunHarnessKind, RunSource,
};
use ta_provider_llm::client::{StreamMessage, StreamToolCallRecord};
use ta_store::{
    PersistenceStore, RunEventRangeQuery, RunProjection, SessionAgentTurnsPageQuery, row_sequence,
};
use taugentic_agent::ForkInitialState;

use super::*;

const FORK_REPLAY_BATCH_LIMIT: usize = 500;
const FORK_REPLAY_TOTAL_LIMIT: usize = 10_000;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(super) fn fork_initial_state_for_run(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
    ) -> Result<Option<ForkInitialState>, RunExecutionError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        let Some(run) = store.run(run_id)? else {
            return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
        };
        if run.session_id != *session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                run.id.as_str().to_string(),
            ));
        }
        build_fork_initial_state(&*store, &run)
    }
}

fn build_fork_initial_state(
    store: &impl PersistenceStore,
    run: &RunProjection,
) -> Result<Option<ForkInitialState>, RunExecutionError> {
    let RunSource::Forked {
        parent_run_id,
        parent_event_seq,
    } = &run.source
    else {
        return Ok(None);
    };

    let Some(parent) = store.run(parent_run_id)? else {
        return Err(RunExecutionError::RunNotFound(
            parent_run_id.as_str().to_string(),
        ));
    };
    if parent.session_id != run.session_id {
        return Err(RunExecutionError::RunSessionMismatch(
            parent.id.as_str().to_string(),
        ));
    }
    if parent.harness != RunHarnessKind::Native {
        return Err(RunExecutionError::RunNotNativeHarness(
            parent.id.as_str().to_string(),
        ));
    }

    fork_initial_state_for_parent(store, &run.session_id, &parent, *parent_event_seq).map(Some)
}

pub(super) fn fork_initial_state_for_parent(
    store: &impl PersistenceStore,
    session_id: &crate::SessionId,
    parent: &RunProjection,
    parent_event_seq: u64,
) -> Result<ForkInitialState, RunExecutionError> {
    let events = read_parent_events_until(store, session_id, &parent.id, parent_event_seq)?;
    validate_fork_boundary(store, session_id, &parent.id, parent_event_seq, &events)?;
    let rows = read_parent_turn_rows_until(store, session_id, &parent.id, parent_event_seq)?;
    Ok(ForkInitialState {
        messages: turn_rows_to_messages(parent.objective.clone(), rows)?,
        provider_session_id: None,
    })
}

fn read_parent_events_until(
    store: &impl PersistenceStore,
    session_id: &crate::SessionId,
    run_id: &RunId,
    boundary: u64,
) -> Result<Vec<ta_store::EventRecord>, RunExecutionError> {
    let mut events = Vec::new();
    let mut after_sequence = None;
    loop {
        if events.len() >= FORK_REPLAY_TOTAL_LIMIT {
            return Err(RunExecutionError::RunForkPointNotFound(format!(
                "{}:{} exceeds replay limit",
                run_id.as_str(),
                boundary
            )));
        }
        let remaining = FORK_REPLAY_TOTAL_LIMIT - events.len();
        let range = store.read_run_events(&RunEventRangeQuery {
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            after_sequence,
            limit: remaining.min(FORK_REPLAY_BATCH_LIMIT),
        })?;
        if range.records.is_empty() {
            break;
        }
        for record in range.records {
            if record.sequence > boundary {
                return Ok(events);
            }
            after_sequence = Some(record.sequence);
            events.push(record);
        }
    }
    Ok(events)
}

fn validate_fork_boundary(
    store: &impl PersistenceStore,
    session_id: &crate::SessionId,
    run_id: &RunId,
    boundary: u64,
    events: &[ta_store::EventRecord],
) -> Result<(), RunExecutionError> {
    let last = events
        .iter()
        .find(|record| record.sequence == boundary)
        .ok_or_else(|| {
            RunExecutionError::RunForkPointNotFound(format!("{}:{}", run_id.as_str(), boundary))
        })?;

    let mut active_assistant_turns = BTreeSet::<Option<AgentStreamTurnId>>::new();
    let mut active_tool_calls = BTreeSet::<(Option<AgentStreamTurnId>, Option<String>)>::new();
    for record in events {
        let DaemonEvent::AgentStream(event) = &record.payload else {
            continue;
        };
        match &event.emission.frame {
            AgentStreamFrame::AssistantTurnStarted => {
                active_assistant_turns.insert(event.emission.turn_id.clone());
            }
            AgentStreamFrame::AssistantTurnCompleted => {
                active_assistant_turns.remove(&event.emission.turn_id);
            }
            AgentStreamFrame::ToolCallStarted { .. } => {
                active_tool_calls.insert((
                    event.emission.turn_id.clone(),
                    event
                        .emission
                        .item_id
                        .as_ref()
                        .map(|id| id.as_str().to_string()),
                ));
            }
            AgentStreamFrame::ToolCallCompleted { .. } => {
                active_tool_calls.remove(&(
                    event.emission.turn_id.clone(),
                    event
                        .emission
                        .item_id
                        .as_ref()
                        .map(|id| id.as_str().to_string()),
                ));
            }
            AgentStreamFrame::AssistantMessageDelta { .. }
            | AgentStreamFrame::ToolCallProgressed { .. }
            | AgentStreamFrame::PendingStateChanged { .. }
            | AgentStreamFrame::TokenUsageUpdated { .. } => {}
        }
    }
    if !active_assistant_turns.is_empty() || !active_tool_calls.is_empty() {
        return Err(not_turn_boundary(run_id, boundary));
    }

    if next_event_continues_same_turn(store, session_id, run_id, boundary, last)? {
        return Err(not_turn_boundary(run_id, boundary));
    }
    Ok(())
}

fn next_event_continues_same_turn(
    store: &impl PersistenceStore,
    session_id: &crate::SessionId,
    run_id: &RunId,
    boundary: u64,
    last: &ta_store::EventRecord,
) -> Result<bool, RunExecutionError> {
    let DaemonEvent::AgentStream(last_stream) = &last.payload else {
        return Ok(false);
    };
    let next = store
        .read_run_events(&RunEventRangeQuery {
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            after_sequence: Some(boundary),
            limit: 1,
        })?
        .records
        .into_iter()
        .next();
    let Some(next) = next else {
        return Ok(false);
    };
    let DaemonEvent::AgentStream(next_stream) = next.payload else {
        return Ok(false);
    };
    Ok(next_stream.emission.turn_id == last_stream.emission.turn_id
        && matches!(
            next_stream.emission.frame,
            AgentStreamFrame::ToolCallStarted { .. }
                | AgentStreamFrame::ToolCallCompleted { .. }
                | AgentStreamFrame::PendingStateChanged { .. }
        ))
}

fn not_turn_boundary(run_id: &RunId, boundary: u64) -> RunExecutionError {
    RunExecutionError::RunForkPointNotTurnBoundary(format!("{}:{}", run_id.as_str(), boundary))
}

fn read_parent_turn_rows_until(
    store: &impl PersistenceStore,
    session_id: &crate::SessionId,
    run_id: &RunId,
    boundary: u64,
) -> Result<Vec<AgentTurnRow>, RunExecutionError> {
    let mut rows = Vec::new();
    let mut before_sequence = boundary.checked_add(1);
    loop {
        if rows.len() >= FORK_REPLAY_TOTAL_LIMIT {
            return Err(RunExecutionError::RunForkPointNotFound(format!(
                "{}:{} exceeds turn-row replay limit",
                run_id.as_str(),
                boundary
            )));
        }
        let remaining = FORK_REPLAY_TOTAL_LIMIT - rows.len();
        let page = store.session_agent_turns_page(&SessionAgentTurnsPageQuery {
            session_id: session_id.clone(),
            before_sequence,
            limit: remaining.min(FORK_REPLAY_BATCH_LIMIT),
        })?;
        if page.rows.is_empty() {
            break;
        }
        for row in page.rows {
            if row_run_id(&row) == run_id && row_sequence(&row) <= boundary {
                rows.push(row);
            }
        }
        let Some(next_before) = page.next_before_sequence else {
            break;
        };
        before_sequence = Some(next_before);
    }
    rows.sort_by_key(row_sequence);
    Ok(rows)
}

fn row_run_id(row: &AgentTurnRow) -> &RunId {
    match row {
        AgentTurnRow::Assistant(row) => &row.run_id,
        AgentTurnRow::ToolCall(row) => &row.run_id,
        AgentTurnRow::PendingState(row) => &row.run_id,
    }
}

fn turn_rows_to_messages(
    parent_objective: String,
    rows: Vec<AgentTurnRow>,
) -> Result<Vec<StreamMessage>, RunExecutionError> {
    let mut messages = vec![StreamMessage::user(parent_objective)];
    let mut index = 0;
    while index < rows.len() {
        let AgentTurnRow::Assistant(assistant) = &rows[index] else {
            return Err(RunExecutionError::RunForkPointNotTurnBoundary(
                "fork snapshot has tool row without assistant row".to_string(),
            ));
        };
        let turn_id = assistant.turn_id.clone();
        let mut tool_rows = Vec::new();
        let mut next = index + 1;
        while next < rows.len() {
            match &rows[next] {
                AgentTurnRow::ToolCall(row) if row.turn_id == turn_id => {
                    tool_rows.push(row.clone());
                    next += 1;
                }
                AgentTurnRow::PendingState(row) if row.turn_id == turn_id => {
                    next += 1;
                }
                _ => break,
            }
        }
        messages.push(StreamMessage::assistant(
            assistant.text.clone(),
            tool_rows
                .iter()
                .map(tool_call_record)
                .collect::<Result<Vec<_>, _>>()?,
        ));
        for row in tool_rows {
            let Some(item_id) = row.item_id else {
                return Err(RunExecutionError::RunForkPointNotTurnBoundary(
                    "fork snapshot has tool row without item id".to_string(),
                ));
            };
            messages.push(StreamMessage::tool(
                item_id.as_str().to_string(),
                row.output,
            ));
        }
        index = next;
    }
    Ok(messages)
}

fn tool_call_record(row: &AgentToolCallRow) -> Result<StreamToolCallRecord, RunExecutionError> {
    let Some(item_id) = &row.item_id else {
        return Err(RunExecutionError::RunForkPointNotTurnBoundary(
            "fork snapshot has tool row without item id".to_string(),
        ));
    };
    Ok(StreamToolCallRecord {
        id: item_id.as_str().to_string(),
        name: row.tool_name.clone(),
        input: serde_json::from_str(&row.input).map_err(|error| {
            RunExecutionError::RunForkPointNotTurnBoundary(format!(
                "fork snapshot has invalid tool input JSON for {}: {error}",
                item_id.as_str()
            ))
        })?,
    })
}
