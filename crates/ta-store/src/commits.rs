use serde::{Deserialize, Serialize};
use ta_protocol::wire::{
    AuthProfileExhaustion, AuthProfileId, DaemonEvent, RunId, SessionId, SessionNextRunSelection,
    WorkspaceFileAttachment,
};

use crate::{
    ArtifactRecord, CheckpointRecord, EventRecord, RunProjection, SessionProjection, StoreError,
    scheduled_run_source,
};

pub(crate) fn scheduled_terminal_state(
    run_id: RunId,
    status: ta_protocol::wire::RunStatus,
) -> Option<ta_protocol::wire::ScheduledWorkOccurrenceState> {
    use ta_protocol::wire::{RunStatus, ScheduledWorkOccurrenceState};
    match status {
        RunStatus::Completed => Some(ScheduledWorkOccurrenceState::Completed { run_id }),
        RunStatus::Failed => Some(ScheduledWorkOccurrenceState::Failed { run_id }),
        RunStatus::BudgetExceeded => Some(ScheduledWorkOccurrenceState::BudgetExceeded { run_id }),
        RunStatus::Cancelled => Some(ScheduledWorkOccurrenceState::Cancelled {
            run_id: Some(run_id),
        }),
        RunStatus::Queued | RunStatus::Running | RunStatus::WaitingForApproval => None,
    }
}

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
    pub user_turn: UserTurnCommit,
    pub events: Vec<DaemonEvent>,
    pub occurred_at_ms: u64,
    pub auth_profile_mutation: AuthProfileCommitMutation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthProfileCommitMutation {
    Unchanged,
    SetExhausted {
        auth_profile_id: AuthProfileId,
        exhaustion: AuthProfileExhaustion,
    },
}

/// The only input that may materialize a durable user row while committing a
/// run transition. Callers must state their intent explicitly; run projection
/// fields are never interpreted as user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserTurnCommit {
    Append {
        text: String,
        attachments: Vec<WorkspaceFileAttachment>,
    },
    NoUserTurn,
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
                if run_event.run_id() != &input.run.id {
                    return Err(StoreError::CommitRunEventMismatch {
                        expected: input.run.id.as_str().to_string(),
                        actual: run_event.run_id().as_str().to_string(),
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

pub(crate) fn validate_run_source_route(
    existing: Option<&RunProjection>,
    next: &RunProjection,
) -> Result<(), StoreError> {
    if existing.is_some_and(|run| run.source.route() != next.source.route()) {
        return Err(StoreError::ImmutableRunSourceRoute {
            run_id: next.id.as_str().to_string(),
        });
    }
    Ok(())
}

/// A schedule occurrence link is immutable once a run exists. Keeping this
/// narrow preserves existing source evolution rules while preventing a normal
/// run from being converted into a scheduled terminal transition (or vice
/// versa) merely because both sources happen to share a route.
pub(crate) fn validate_scheduled_run_source_link(
    existing: Option<&RunProjection>,
    next: &RunProjection,
) -> Result<(), StoreError> {
    let Some(existing) = existing else {
        return Ok(());
    };
    match (scheduled_run_source(existing), scheduled_run_source(next)) {
        (Some((existing_work, existing_occurrence)), Some((next_work, next_occurrence)))
            if existing_work == next_work && existing_occurrence == next_occurrence =>
        {
            Ok(())
        }
        (None, None) => Ok(()),
        _ => Err(StoreError::ScheduledWorkRunSourceMismatch {
            occurrence_id: scheduled_run_source(next)
                .map(|(_, occurrence)| occurrence.as_str().to_string())
                .unwrap_or_else(|| "none".to_string()),
        }),
    }
}

pub(crate) fn validate_auth_profile_mutation(
    input: &CommitRunTransition,
) -> Result<(), StoreError> {
    let AuthProfileCommitMutation::SetExhausted {
        auth_profile_id,
        exhaustion,
    } = &input.auth_profile_mutation
    else {
        return Ok(());
    };
    if input.run.source.route().auth_profile_id.as_ref() != Some(auth_profile_id) {
        return Err(StoreError::AuthProfileMutationRouteMismatch {
            run_id: input.run.id.as_str().to_string(),
        });
    }
    let matches_terminal_event = input.events.iter().any(|event| {
        matches!(event,
            DaemonEvent::Run(ta_protocol::wire::RunEvent::Status(status))
                if status.status() == ta_protocol::wire::RunStatus::Failed
                    && status.auth_profile_exhaustion() == Some(*exhaustion)
        )
    });
    if !matches_terminal_event {
        return Err(StoreError::AuthProfileMutationMissingTerminalStatus {
            run_id: input.run.id.as_str().to_string(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSessionOpen {
    pub session: SessionProjection,
    pub occurred_at_ms: u64,
}

/// One atomic session-open operation that also writes the session's explicit
/// navigation metadata. The application assembles the validated final
/// navigation state before this reaches the store; the store owns making both
/// projections durable together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSessionOpenWithNavigation {
    pub session: SessionProjection,
    pub owner_principal_id: String,
    pub navigation: crate::NavigationState,
    pub occurred_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSessionNextRunSelection {
    pub session_id: SessionId,
    pub selection: SessionNextRunSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOpenCommitResult {
    pub commit: CommitBoundary,
    pub session: SessionProjection,
    pub event: EventRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNextRunSelectionCommitResult {
    pub session: SessionProjection,
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
