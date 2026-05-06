use ta_protocol::wire::{
    AgentStreamEvent, AgentStreamFrame, AgentToolCallOutcome, ApprovalEvent, ApprovalId,
    ApprovalRequest, ApprovalScope, ArtifactEvent, ArtifactId, ArtifactKind, ArtifactSummary,
    DaemonEvent, RunEvent, RunId, RunStatus, SessionEvent, SessionId, SessionStatus,
};

use super::*;
use crate::PersistenceStore;

fn ok<T>(result: Result<T, StoreError>) -> T {
    result.expect("store read should succeed")
}

fn some<T>(result: Result<Option<T>, StoreError>) -> T {
    ok(result).expect("record should exist")
}

fn agent_stream_event(run_id: &RunId, frame: AgentStreamFrame) -> DaemonEvent {
    DaemonEvent::AgentStream(AgentStreamEvent {
        run_id: run_id.clone(),
        emission: ta_protocol::wire::StreamEmission {
            turn_id: None,
            item_id: None,
            fragment_sequence: None,
            frame,
        },
    })
}

mod agent_turns;
mod artifacts;
mod commits;
mod repositories;
mod work_items;
