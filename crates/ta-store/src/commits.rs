use serde::{Deserialize, Serialize};
use ta_protocol::wire::{DaemonEvent, SessionId};

use crate::{
    ArtifactRecord, CheckpointRecord, EventRecord, RunProjection, SessionProjection, StoreError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitBoundary {
    pub id: u64,
    pub first_sequence: u64,
    pub last_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRunTransition {
    pub session_id: SessionId,
    pub run: RunProjection,
    pub events: Vec<DaemonEvent>,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitStartupReconciliation {
    pub transitions: Vec<CommitRunTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTransitionCommitResult {
    pub commit: CommitBoundary,
    pub session: SessionProjection,
    pub run: RunProjection,
    pub events: Vec<EventRecord>,
    pub persisted_events: Vec<EventRecord>,
}

pub(crate) fn validate_run_transition_events(
    input: &CommitRunTransition,
) -> Result<(), StoreError> {
    for event in &input.events {
        match event {
            DaemonEvent::Run(run_event) => {
                if run_event.run_id != input.run.id {
                    return Err(StoreError::CommitRunEventMismatch {
                        expected: input.run.id.as_str().to_string(),
                        actual: run_event.run_id.as_str().to_string(),
                    });
                }
            }
            DaemonEvent::RunReconciledOnStartup(event) => {
                if event.run_id != input.run.id {
                    return Err(StoreError::CommitRunEventMismatch {
                        expected: input.run.id.as_str().to_string(),
                        actual: event.run_id.as_str().to_string(),
                    });
                }
            }
            DaemonEvent::AgentStream(stream_event) => {
                if stream_event.run_id != input.run.id {
                    return Err(StoreError::CommitRunEventMismatch {
                        expected: input.run.id.as_str().to_string(),
                        actual: stream_event.run_id.as_str().to_string(),
                    });
                }
            }
            DaemonEvent::TokenUsageRecorded(event) => {
                if event.run_id != input.run.id {
                    return Err(StoreError::CommitRunEventMismatch {
                        expected: input.run.id.as_str().to_string(),
                        actual: event.run_id.as_str().to_string(),
                    });
                }
            }
            DaemonEvent::Conflict(ta_protocol::wire::ConflictEvent::Warning { run_id, .. }) => {
                if run_id != &input.run.id {
                    return Err(StoreError::CommitRunEventMismatch {
                        expected: input.run.id.as_str().to_string(),
                        actual: run_id.as_str().to_string(),
                    });
                }
            }
            DaemonEvent::Budget(ta_protocol::wire::BudgetEvent::Exceeded { event }) => {
                if event.run_id != input.run.id {
                    return Err(StoreError::CommitRunEventMismatch {
                        expected: input.run.id.as_str().to_string(),
                        actual: event.run_id.as_str().to_string(),
                    });
                }
            }
            DaemonEvent::Approval(_) | DaemonEvent::Artifact(_) | DaemonEvent::Session(_) => {}
            DaemonEvent::ContextReceipt(_) => {}
        }
    }
    Ok(())
}

pub(crate) fn validate_run_execution_context(
    existing: Option<&RunProjection>,
    next: &RunProjection,
) -> Result<(), StoreError> {
    if existing.is_some_and(|run| run.execution_context != next.execution_context) {
        return Err(StoreError::ImmutableRunExecutionContext {
            run_id: next.id.as_str().to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSessionOpen {
    pub session: SessionProjection,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOpenCommitResult {
    pub commit: CommitBoundary,
    pub session: SessionProjection,
    pub event: EventRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitArtifactPublish {
    pub artifact: ArtifactRecord,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPublishCommitResult {
    pub commit: CommitBoundary,
    pub artifact: ArtifactRecord,
    pub event: EventRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitReceiptEvent {
    pub session_id: SessionId,
    pub event: DaemonEvent,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptEventCommitResult {
    pub commit: CommitBoundary,
    pub event: EventRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitCheckpointPersist {
    pub checkpoint: CheckpointRecord,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointPersistCommitResult {
    pub commit: CommitBoundary,
    pub checkpoint: CheckpointRecord,
}

#[cfg(test)]
mod tests {
    use ta_protocol::wire::{
        PermissionPolicy, RunHarnessKind, RunId, RunStatus, RuntimeProfileId, SessionId,
    };

    use super::*;

    #[test]
    fn run_execution_context_cannot_change_after_creation() {
        let existing = RunProjection {
            id: RunId::new("run-context-immutable").expect("run id"),
            session_id: SessionId::new("session-context-immutable").expect("session id"),
            runtime_profile_id: RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile id"),
            objective: "Keep the resolved context".to_string(),
            status: RunStatus::Running,
            harness: RunHarnessKind::Native,
            source: crate::default_test_run_source(),
            execution_context: crate::default_test_execution_context(),
            result: None,
            contract_violation: None,
            started_at_ms: None,
            ended_at_ms: None,
            last_event_seq: None,
            workspace_info: None,
            claimed_files: Vec::new(),
            conflict_summary: None,
        };
        let mut next = existing.clone();
        next.execution_context.permission_policy = PermissionPolicy::ReadOnly;

        assert_eq!(
            validate_run_execution_context(Some(&existing), &next),
            Err(StoreError::ImmutableRunExecutionContext {
                run_id: existing.id.as_str().to_string(),
            })
        );
    }
}
