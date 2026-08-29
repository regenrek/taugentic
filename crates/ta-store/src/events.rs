use serde::{Deserialize, Serialize};
use ta_protocol::wire::{
    AgentStreamFrame, ApprovalId, ApprovalRequest, DaemonEvent, DaemonEventKind, RunId, SessionId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPersistence {
    Durable,
    Transient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    pub sequence: u64,
    pub session_id: SessionId,
    pub occurred_at_ms: u64,
    pub payload: DaemonEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionApprovalQuery {
    pub session_id: SessionId,
    pub run_id: Option<RunId>,
    pub approval_id: Option<ApprovalId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionApprovalLookup {
    Pending(ApprovalRequest),
    Resolved,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEventPageQuery {
    pub session_id: SessionId,
    pub before_sequence: Option<u64>,
    pub limit: usize,
    pub kinds: Vec<DaemonEventKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEventPage {
    pub records: Vec<EventRecord>,
    pub next_before_sequence: Option<u64>,
    pub latest_sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEventRangeQuery {
    pub session_id: SessionId,
    pub after_sequence: Option<u64>,
    pub up_to_sequence: Option<u64>,
    pub kinds: Vec<DaemonEventKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEventRange {
    pub records: Vec<EventRecord>,
    pub latest_sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEventRangeQuery {
    pub session_id: SessionId,
    pub run_id: RunId,
    pub after_sequence: Option<u64>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEventRange {
    pub records: Vec<EventRecord>,
    pub latest_sequence: Option<u64>,
}

pub fn event_run_id(event: &DaemonEvent) -> Option<&RunId> {
    match event {
        DaemonEvent::Run(event) => Some(event.run_id()),
        DaemonEvent::RunReconciledOnStartup(event) => Some(&event.run_id),
        DaemonEvent::Approval(approval) => match approval {
            ta_protocol::wire::ApprovalEvent::Requested { request } => Some(&request.run_id),
            ta_protocol::wire::ApprovalEvent::Resolved { resolution } => Some(&resolution.run_id),
        },
        DaemonEvent::Artifact(event) => Some(&event.artifact.run_id),
        DaemonEvent::ContextReceipt(event) => match event {
            ta_protocol::wire::ContextReceiptEvent::Created { receipt }
            | ta_protocol::wire::ContextReceiptEvent::Promoted { receipt }
            | ta_protocol::wire::ContextReceiptEvent::Quarantined { receipt } => {
                Some(&receipt.run_id)
            }
        },
        DaemonEvent::AgentStream(event) => Some(&event.run_id),
        DaemonEvent::TokenUsageRecorded(event) => Some(&event.run_id),
        DaemonEvent::Conflict(ta_protocol::wire::ConflictEvent::Warning { run_id, .. }) => {
            Some(run_id)
        }
        DaemonEvent::Budget(ta_protocol::wire::BudgetEvent::Exceeded { event }) => {
            Some(&event.run_id)
        }
        DaemonEvent::Session(_) => None,
    }
}

pub(crate) fn run_event_range_from_records(
    records: impl IntoIterator<Item = EventRecord>,
    query: &RunEventRangeQuery,
) -> RunEventRange {
    let mut latest_sequence = None;
    let mut selected = Vec::with_capacity(query.limit.min(1024));

    for record in records {
        if record.session_id != query.session_id {
            continue;
        }
        if event_run_id(&record.payload) != Some(&query.run_id) {
            continue;
        }

        latest_sequence = Some(record.sequence);
        if query
            .after_sequence
            .is_some_and(|after_sequence| record.sequence <= after_sequence)
        {
            continue;
        }
        if selected.len() >= query.limit {
            continue;
        }

        selected.push(record);
    }

    RunEventRange {
        records: selected,
        latest_sequence,
    }
}

/// Classifies whether an event belongs to the durable activity timeline or only
/// to the live stream lane.
///
/// Durable frames are persisted and participate in replay cursors. Transient
/// frames are delivered live and may be evicted from the in-memory backlog
/// under pressure, after which reconnect falls back to the normal history-gap
/// path instead of reconstructing in-flight text.
///
/// This matches the current narrow-lossless tier we want: turn/tool lifecycle
/// markers stay replayable, while assistant deltas and tool progress remain
/// best-effort live transport.
pub fn event_persistence(event: &DaemonEvent) -> EventPersistence {
    match event {
        DaemonEvent::AgentStream(event) => match &event.emission.frame {
            AgentStreamFrame::AssistantTurnStarted
            | AgentStreamFrame::AssistantTurnCompleted
            | AgentStreamFrame::ToolCallStarted { .. }
            | AgentStreamFrame::ToolCallCompleted { .. }
            | AgentStreamFrame::PendingStateChanged { .. }
            | AgentStreamFrame::TokenUsageUpdated { .. } => EventPersistence::Durable,
            AgentStreamFrame::AssistantMessageDelta { .. }
            | AgentStreamFrame::ToolCallProgressed { .. } => EventPersistence::Transient,
        },
        DaemonEvent::Approval(_)
        | DaemonEvent::ContextReceipt(_)
        | DaemonEvent::Conflict(_)
        | DaemonEvent::Budget(_)
        | DaemonEvent::Artifact(_)
        | DaemonEvent::RunReconciledOnStartup(_)
        | DaemonEvent::TokenUsageRecorded(_)
        | DaemonEvent::Run(_)
        | DaemonEvent::Session(_) => EventPersistence::Durable,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EventPersistence, EventRecord, RunEventRangeQuery, event_persistence,
        run_event_range_from_records,
    };
    use ta_protocol::wire::{
        AgentStreamEvent, AgentStreamFrame, AgentToolCallOutcome, DaemonEvent, RunEvent, RunId,
        RunStatus, RuntimeLanePendingState, SessionId, StreamEmission,
    };

    fn agent_stream_event(frame: AgentStreamFrame) -> DaemonEvent {
        DaemonEvent::AgentStream(AgentStreamEvent {
            run_id: RunId::new("run-1").expect("run id"),
            emission: StreamEmission {
                turn_id: None,
                item_id: None,
                fragment_sequence: None,
                frame,
            },
        })
    }

    #[test]
    fn event_persistence_classifies_agent_stream_frames() {
        let cases = [
            (
                agent_stream_event(AgentStreamFrame::AssistantTurnStarted),
                EventPersistence::Durable,
            ),
            (
                agent_stream_event(AgentStreamFrame::AssistantMessageDelta {
                    delta: "partial".to_string(),
                }),
                EventPersistence::Transient,
            ),
            (
                agent_stream_event(AgentStreamFrame::AssistantTurnCompleted),
                EventPersistence::Durable,
            ),
            (
                agent_stream_event(AgentStreamFrame::ToolCallStarted {
                    tool_name: "shell".to_string(),
                    input: "{}".to_string(),
                }),
                EventPersistence::Durable,
            ),
            (
                agent_stream_event(AgentStreamFrame::ToolCallProgressed {
                    delta: "stdout".to_string(),
                }),
                EventPersistence::Transient,
            ),
            (
                agent_stream_event(AgentStreamFrame::ToolCallCompleted {
                    outcome: AgentToolCallOutcome::Completed,
                }),
                EventPersistence::Durable,
            ),
            (
                agent_stream_event(AgentStreamFrame::PendingStateChanged {
                    state: RuntimeLanePendingState::WaitingForApproval,
                }),
                EventPersistence::Durable,
            ),
        ];

        for (event, expected) in cases {
            assert_eq!(event_persistence(&event), expected);
        }
    }

    #[test]
    fn run_event_range_filters_after_sequence_and_limit() {
        let session_id = SessionId::new("session-1").expect("session id");
        let run_id = RunId::new("run-1").expect("run id");
        let other_run_id = RunId::new("run-2").expect("run id");
        let event = |sequence, run_id: &RunId| EventRecord {
            sequence,
            session_id: session_id.clone(),
            occurred_at_ms: sequence * 10,
            payload: DaemonEvent::Run(
                RunEvent::active(run_id.clone(), RunStatus::Running, None, None, None)
                    .expect("active status"),
            ),
        };

        let range = run_event_range_from_records(
            [
                event(1, &run_id),
                event(2, &other_run_id),
                event(3, &run_id),
                event(4, &run_id),
            ],
            &RunEventRangeQuery {
                session_id,
                run_id,
                after_sequence: Some(1),
                limit: 1,
            },
        );

        assert_eq!(
            range
                .records
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![3]
        );
        assert_eq!(range.latest_sequence, Some(4));
    }
}
