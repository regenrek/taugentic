use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::Connection;
use ta_protocol::wire::{
    AgentStreamEvent, AgentStreamFrame, AgentToolCallOutcome, ApprovalDecision, ApprovalEvent,
    ApprovalId, ApprovalRequest, ApprovalScope, ArtifactId, ArtifactKind, DaemonEvent,
    DaemonEventKind, RunHarnessKind, RunId, RunSource, RunStatus, SessionId, SessionStatus,
};

use super::*;

fn ok<T>(result: Result<T, StoreError>) -> T {
    result.expect("sqlite store read should succeed")
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

fn test_db_path(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("taugentic-sqlite-store-{label}-{nanos}.sqlite3"))
}

mod agent_turns;
mod artifacts;
mod checkpoints;
mod commits;
mod events;
mod migrations;
mod principals;
mod runs_list;
mod sessions;
mod work_items;
