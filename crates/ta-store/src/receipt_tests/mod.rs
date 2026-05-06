use ta_protocol::wire::{
    AgentStreamTurnId, ArtifactId, ReceiptKind, ReceiptProvenance, RunId, SessionId,
};

use crate::CreateReceipt;

mod list_filter;
mod parity;
mod provenance;
mod state_machine;

fn artifact_receipt(
    session_id: &SessionId,
    run_id: &RunId,
    parent_run_id: Option<RunId>,
    kind: ReceiptKind,
    artifact_id: &str,
) -> CreateReceipt {
    CreateReceipt {
        session_id: session_id.clone(),
        run_id: run_id.clone(),
        parent_run_id,
        kind,
        provenance: ReceiptProvenance {
            artifact_id: Some(ArtifactId::new(artifact_id).expect("artifact id")),
            agent_turn_id: None,
            event_seq: None,
            stream_cursor: None,
        },
        title: Some(format!("receipt {artifact_id}")),
        summary: None,
    }
}

fn stream_cursor_only_receipt(
    session_id: &SessionId,
    run_id: &RunId,
    stream_cursor: &str,
) -> CreateReceipt {
    CreateReceipt {
        session_id: session_id.clone(),
        run_id: run_id.clone(),
        parent_run_id: None,
        kind: ReceiptKind::Risk,
        provenance: ReceiptProvenance {
            artifact_id: None,
            agent_turn_id: None,
            event_seq: None,
            stream_cursor: Some(stream_cursor.to_string()),
        },
        title: Some(format!("stream cursor {stream_cursor}")),
        summary: None,
    }
}

fn artifact_receipt_with_stream_cursor(
    session_id: &SessionId,
    run_id: &RunId,
    kind: ReceiptKind,
    artifact_id: &str,
    stream_cursor: &str,
) -> CreateReceipt {
    let mut receipt = artifact_receipt(session_id, run_id, None, kind, artifact_id);
    receipt.provenance.stream_cursor = Some(stream_cursor.to_string());
    receipt
}

fn event_turn_receipt(
    session_id: &SessionId,
    run_id: &RunId,
    event_seq: u64,
    agent_turn_id: &str,
    stream_cursor: Option<&str>,
) -> CreateReceipt {
    CreateReceipt {
        session_id: session_id.clone(),
        run_id: run_id.clone(),
        parent_run_id: None,
        kind: ReceiptKind::Summary,
        provenance: ReceiptProvenance {
            artifact_id: None,
            agent_turn_id: Some(AgentStreamTurnId::new(agent_turn_id).expect("turn id")),
            event_seq: Some(event_seq),
            stream_cursor: stream_cursor.map(str::to_string),
        },
        title: Some(format!("event turn {agent_turn_id}")),
        summary: None,
    }
}

fn mixed_provenance_receipt(
    session_id: &SessionId,
    run_id: &RunId,
    artifact_id: &str,
    event_seq: u64,
    agent_turn_id: &str,
) -> CreateReceipt {
    CreateReceipt {
        session_id: session_id.clone(),
        run_id: run_id.clone(),
        parent_run_id: None,
        kind: ReceiptKind::Summary,
        provenance: ReceiptProvenance {
            artifact_id: Some(ArtifactId::new(artifact_id).expect("artifact id")),
            agent_turn_id: Some(AgentStreamTurnId::new(agent_turn_id).expect("turn id")),
            event_seq: Some(event_seq),
            stream_cursor: None,
        },
        title: None,
        summary: None,
    }
}

fn make_session_id(value: &str) -> SessionId {
    SessionId::new(value).expect("session id")
}

fn make_run_id(value: &str) -> RunId {
    RunId::new(value).expect("run id")
}
