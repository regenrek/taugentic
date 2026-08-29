use serde::{Deserialize, Serialize};
use ta_protocol::wire::{
    SessionId, ThreadWorkspaceMutation, ThreadWorkspaceResult, ThreadWorkspaceWorkLogEntry,
    ThreadWorkspaceWorkLogKind,
};

use crate::StoreError;

/// Canonical durable metadata for a single conversation. It contains
/// references only; transcript rows and event payloads retain their owners.
pub type ThreadWorkspaceRecord = ThreadWorkspaceResult;

/// The only mutable input to a thread workspace. The store appends these in
/// order and derives `ThreadWorkspaceRecord`; callers cannot write a cached
/// projection directly.
pub type ThreadWorkspaceEvent = ThreadWorkspaceMutation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadWorkspaceEventRecord {
    pub sequence: u64,
    pub occurred_at_ms: u64,
    pub payload: ThreadWorkspaceMutation,
}

pub fn validate_thread_workspace(record: &ThreadWorkspaceRecord) -> Result<(), StoreError> {
    if record.pins.iter().any(|pin| pin.cursor.sequence == 0) {
        return Err(StoreError::AgentTurnProjectionViolation {
            detail: "thread workspace pin cursor must be durable".to_string(),
        });
    }
    let mut previous = None;
    for pin in &record.pins {
        if previous.is_some_and(|sequence| pin.cursor.sequence <= sequence) {
            return Err(StoreError::AgentTurnProjectionViolation {
                detail: "thread workspace pins must be ordered by durable cursor".to_string(),
            });
        }
        previous = Some(pin.cursor.sequence);
    }
    Ok(())
}

pub fn derive_thread_workspace(
    session_id: SessionId,
    events: &[ThreadWorkspaceEventRecord],
) -> Result<ThreadWorkspaceRecord, StoreError> {
    let mut record = ThreadWorkspaceRecord {
        session_id,
        goal: String::new(),
        plan: String::new(),
        notes: String::new(),
        pins: Vec::new(),
        recap: String::new(),
        work_log: Vec::new(),
    };
    let mut previous = 0;
    for event in events {
        if event.sequence <= previous {
            return Err(StoreError::AgentTurnProjectionViolation {
                detail: "thread workspace events must be append ordered".to_string(),
            });
        }
        previous = event.sequence;
        match &event.payload {
            ThreadWorkspaceEvent::GoalSet { value } => {
                record.goal = value.clone();
                record.work_log.push(ThreadWorkspaceWorkLogEntry {
                    sequence: event.sequence,
                    occurred_at_ms: event.occurred_at_ms,
                    kind: ThreadWorkspaceWorkLogKind::GoalSet,
                });
            }
            ThreadWorkspaceEvent::PlanSet { value } => {
                record.plan = value.clone();
                record.work_log.push(ThreadWorkspaceWorkLogEntry {
                    sequence: event.sequence,
                    occurred_at_ms: event.occurred_at_ms,
                    kind: ThreadWorkspaceWorkLogKind::PlanSet,
                });
            }
            ThreadWorkspaceEvent::NotesSet { value } => {
                record.notes = value.clone();
                record.work_log.push(ThreadWorkspaceWorkLogEntry {
                    sequence: event.sequence,
                    occurred_at_ms: event.occurred_at_ms,
                    kind: ThreadWorkspaceWorkLogKind::NotesSet,
                });
            }
            ThreadWorkspaceEvent::RecapSet { value } => {
                record.recap = value.clone();
                record.work_log.push(ThreadWorkspaceWorkLogEntry {
                    sequence: event.sequence,
                    occurred_at_ms: event.occurred_at_ms,
                    kind: ThreadWorkspaceWorkLogKind::RecapSet,
                });
            }
            ThreadWorkspaceEvent::PinAdded { pin } => {
                if record
                    .pins
                    .iter()
                    .any(|current| current.cursor == pin.cursor)
                {
                    return Err(StoreError::AgentTurnProjectionViolation {
                        detail: "thread workspace pin already exists".to_string(),
                    });
                }
                record.pins.push(pin.clone());
                // The append stream records user intent, not display order. A
                // later pin must not prevent selecting an earlier durable turn;
                // projections always use this one canonical cursor order.
                record.pins.sort_by_key(|current| current.cursor.sequence);
                record.work_log.push(ThreadWorkspaceWorkLogEntry {
                    sequence: event.sequence,
                    occurred_at_ms: event.occurred_at_ms,
                    kind: ThreadWorkspaceWorkLogKind::PinAdded,
                });
            }
            ThreadWorkspaceEvent::PinRemoved { cursor } => {
                let before = record.pins.len();
                record.pins.retain(|pin| pin.cursor != *cursor);
                if before == record.pins.len() {
                    return Err(StoreError::AgentTurnProjectionViolation {
                        detail: "thread workspace pin does not exist".to_string(),
                    });
                }
                record.work_log.push(ThreadWorkspaceWorkLogEntry {
                    sequence: event.sequence,
                    occurred_at_ms: event.occurred_at_ms,
                    kind: ThreadWorkspaceWorkLogKind::PinRemoved,
                });
            }
        }
    }
    validate_thread_workspace(&record)?;
    Ok(record)
}
