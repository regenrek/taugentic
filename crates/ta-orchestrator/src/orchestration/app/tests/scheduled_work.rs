use std::time::{SystemTime, UNIX_EPOCH};

use ta_protocol::wire::{
    CancelScheduledWorkRequest, CreateScheduledWorkRequest, ScheduledWorkOccurrenceState,
    WorkspaceMode, WorktreeCleanupPolicy,
};
use ta_store::{
    EventLogRepository, ProjectionRepository, ScheduledWorkRepository, SessionApprovalQuery,
};

use super::*;

fn scheduled_request(
    service: &AppService,
    objective: &str,
    due_at_ms: u64,
) -> CreateScheduledWorkRequest {
    let selection = crate::orchestration::test_runtime_selection(service, "runtime-openai-safe");
    CreateScheduledWorkRequest {
        objective: objective.to_string(),
        selection,
        due_at_ms,
    }
}

#[test]
fn scheduled_work_deadline_rearms_after_pending_cancel() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Scheduled deadline cancel");
    let created = service
        .create_scheduled_work(
            &session.id,
            scheduled_request(&service, "Later work", u64::MAX),
        )
        .expect("scheduled work should persist");
    assert_eq!(
        service
            .next_scheduled_work_deadline_ms()
            .expect("deadline should query"),
        Some(u64::MAX)
    );

    service
        .cancel_scheduled_work(
            &session.id,
            &approval_actor(),
            &CancelScheduledWorkRequest {
                occurrence_id: created.occurrence.id,
            },
        )
        .expect("pending scheduled work should cancel");
    assert_eq!(
        service
            .next_scheduled_work_deadline_ms()
            .expect("deadline should query"),
        None
    );
}

#[test]
fn scheduled_work_overdue_pending_is_visible_to_deadline_owner() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Scheduled overdue");
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    service
        .create_scheduled_work(
            &session.id,
            scheduled_request(&service, "Overdue work", now_ms - 1),
        )
        .expect("scheduled work should persist");
    assert!(
        service
            .next_scheduled_work_deadline_ms()
            .expect("deadline should query")
            .expect("pending occurrence")
            <= now_ms
    );
}

#[test]
fn scheduled_work_overdue_publishes_once_and_enters_scheduler_once() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Scheduled overdue once");
    let due_at_ms = current_time_ms().saturating_sub(1);
    let created = service
        .create_scheduled_work(
            &session.id,
            scheduled_request(&service, "Overdue once", due_at_ms),
        )
        .expect("scheduled work should persist");

    service
        .process_due_scheduled_work()
        .expect("overdue occurrence should publish and schedule");
    service
        .process_due_scheduled_work()
        .expect("terminal snapshot must not replay");

    let store = service.store.lock().expect("store");
    let occurrence = store
        .scheduled_work_occurrence(&created.occurrence.id)
        .expect("occurrence query")
        .expect("occurrence exists");
    let runs = store.runs().expect("runs query");
    assert!(matches!(
        occurrence.state,
        ScheduledWorkOccurrenceState::Claimed { .. }
    ));
    assert_eq!(runs.len(), 1, "only the first due pass may publish a run");
    assert_eq!(runs[0].status, RunStatus::WaitingForApproval);
    let approvals = store
        .approvals_for_session(&SessionApprovalQuery {
            session_id: session.id.clone(),
            run_id: Some(runs[0].id.clone()),
            approval_id: None,
        })
        .expect("approval query should succeed");
    assert_eq!(
        approvals.len(),
        1,
        "frozen approval policy must create one approval"
    );
    assert_eq!(
        runs[0].status,
        RunStatus::WaitingForApproval,
        "provider dispatch must not begin before the frozen approval is resolved"
    );
}

#[test]
fn scheduled_work_deadline_stale_cancellation_preserves_later_due_occurrence() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Scheduled stale cancellation");
    let due_at_ms = current_time_ms().saturating_sub(1);
    let stale = service
        .create_scheduled_work(
            &session.id,
            scheduled_request(&service, "Cancelled stale", due_at_ms),
        )
        .expect("stale occurrence should persist");
    let later = service
        .create_scheduled_work(
            &session.id,
            scheduled_request(&service, "Still due", due_at_ms),
        )
        .expect("later occurrence should persist");

    service
        .cancel_scheduled_work(
            &session.id,
            &approval_actor(),
            &CancelScheduledWorkRequest {
                occurrence_id: stale.occurrence.id.clone(),
            },
        )
        .expect("pending stale occurrence should cancel");
    service
        .process_due_scheduled_work_occurrence(stale.occurrence.clone())
        .expect("durably cancelled stale snapshot must not kill the deadline owner");
    service
        .process_due_scheduled_work()
        .expect("later due occurrence should still be processed");

    let store = service.store.lock().expect("store");
    let stale = store
        .scheduled_work_occurrence(&stale.occurrence.id)
        .expect("stale occurrence query")
        .expect("stale occurrence exists");
    let later = store
        .scheduled_work_occurrence(&later.occurrence.id)
        .expect("later occurrence query")
        .expect("later occurrence exists");
    assert!(matches!(
        stale.state,
        ScheduledWorkOccurrenceState::Cancelled { run_id: None }
    ));
    assert!(matches!(
        later.state,
        ScheduledWorkOccurrenceState::Claimed { .. }
    ));
}

#[test]
fn scheduled_work_queue_full_terminalizes_linked_run_and_occurrence() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Scheduled queue full");
    let due_at_ms = current_time_ms().saturating_sub(1);
    let mut created = Vec::new();
    // One active slot, exactly MAX queued slots, then one additional due
    // occurrence that must converge through the existing QueueFull cancel.
    for index in 0..=crate::MAX_QUEUED_RUNS_PER_SESSION + 1 {
        created.push(
            service
                .create_scheduled_work(
                    &session.id,
                    scheduled_request(&service, &format!("Queue {index}"), due_at_ms),
                )
                .expect("scheduled work should persist"),
        );
    }

    service
        .process_due_scheduled_work()
        .expect("queue saturation must converge rather than fail the deadline owner");

    let store = service.store.lock().expect("store");
    let terminal_occurrence = created
        .iter()
        .map(|created| {
            store
                .scheduled_work_occurrence(&created.occurrence.id)
                .expect("occurrence query")
                .expect("occurrence exists")
        })
        .find(|occurrence| {
            matches!(
                occurrence.state,
                ScheduledWorkOccurrenceState::Cancelled { run_id: Some(_) }
            )
        })
        .expect("one saturated occurrence must be terminally linked");
    let run_id = match &terminal_occurrence.state {
        ScheduledWorkOccurrenceState::Cancelled {
            run_id: Some(run_id),
        } => run_id,
        state => panic!("queue-full occurrence must be terminally linked, got {state:?}"),
    };
    let run = store
        .run(run_id)
        .expect("run query")
        .expect("linked run exists");
    assert_eq!(run.status, RunStatus::Cancelled);
}

#[test]
fn scheduled_work_cleanup_required_is_terminal_and_never_enqueued() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Scheduled cleanup terminal");
    let due_at_ms = current_time_ms().saturating_sub(1);
    let created = service
        .create_scheduled_work(
            &session.id,
            scheduled_request(&service, "Cleanup terminal", due_at_ms),
        )
        .expect("scheduled work should persist");
    let run_id = RunId::new("run-scheduled-cleanup-terminal").expect("run id");
    let resource = ta_protocol::wire::ScheduledWorkUnpublishedResource {
        parent_repo: "/".to_string(),
        worktree_path: "/tmp/taugentic-worktrees/run-scheduled-cleanup-terminal".to_string(),
        branch: "ta/capsule-run-scheduled-cleanup-terminal".to_string(),
        cleanup_policy: WorktreeCleanupPolicy::DeleteOnSuccess,
    };
    {
        let mut store = service.store.lock().expect("store");
        store
            .reserve_scheduled_work_occurrence(ta_store::ReserveScheduledWorkOccurrence {
                scheduled_work_id: created.definition.id.clone(),
                occurrence_id: created.occurrence.id.clone(),
                run_id: run_id.clone(),
            })
            .expect("reserve occurrence");
        store
            .finalize_preparing_scheduled_work_cleanup(
                &created.occurrence.id,
                &run_id,
                ta_protocol::wire::ScheduledWorkPreparationTerminal::Failed,
                resource,
                "prepare failed".to_string(),
                Err("cleanup failed".to_string()),
            )
            .expect("visible cleanup terminal");
    }

    service
        .process_due_scheduled_work()
        .expect("cleanup-required occurrence must not be retried");
    let store = service.store.lock().expect("store");
    let occurrence = store
        .scheduled_work_occurrence(&created.occurrence.id)
        .expect("occurrence query")
        .expect("occurrence exists");
    assert!(matches!(
        occurrence.state,
        ScheduledWorkOccurrenceState::CleanupRequired { .. }
    ));
    assert!(store.runs().expect("runs query").is_empty());
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64
}

#[test]
fn scheduled_work_create_freezes_explicit_selection_server_side() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Scheduled create freezes selection");
    let selection = crate::orchestration::test_runtime_selection(&service, "runtime-openai-safe");

    let created = service
        .create_scheduled_work(
            &session.id,
            CreateScheduledWorkRequest {
                objective: "Freeze this exact selection".to_string(),
                selection: selection.clone(),
                due_at_ms: u64::MAX,
            },
        )
        .expect("create should freeze daemon-owned execution input");

    assert_eq!(
        created.definition.route.runtime_profile_id,
        selection.runtime_profile_id
    );
    assert_eq!(created.definition.route.model_id, selection.model_id);
    assert_eq!(
        created.definition.route.auth_profile_id,
        selection.auth_profile_id
    );
    assert_eq!(
        created.definition.execution_request.workspace_id,
        ta_store::default_test_workspace_id(),
        "the attached session workspace is frozen by the daemon"
    );
    assert_eq!(
        created.definition.execution_request.workspace_mode,
        WorkspaceMode::WorkspaceWrite,
        "the canonical workspace-write policy compiler owns scheduled create"
    );
}

#[test]
fn scheduled_work_create_rejects_realtime_selection_without_durable_definition() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Scheduled create rejects realtime");
    let selection =
        crate::orchestration::test_runtime_selection(&service, "runtime-openai-realtime");

    let error = service
        .create_scheduled_work(
            &session.id,
            CreateScheduledWorkRequest {
                objective: "Do not schedule realtime".to_string(),
                selection,
                due_at_ms: u64::MAX,
            },
        )
        .expect_err("realtime profiles have no scheduled-work execution route");
    assert!(error.to_string().contains("realtime voice"));
    assert!(
        service
            .store
            .lock()
            .expect("store")
            .scheduled_work_occurrences()
            .expect("scheduled occurrences")
            .is_empty()
    );
}

#[test]
fn scheduled_work_create_has_no_run_allocation_or_dispatch() {
    let service = AppService::bootstrap().expect("app service should boot");
    let session = open_test_session(&service, "Scheduled create stays inert");

    service
        .create_scheduled_work(
            &session.id,
            scheduled_request(&service, "Inert create", u64::MAX),
        )
        .expect("create should persist only the frozen definition");

    let store = service.store.lock().expect("store");
    assert!(store.runs().expect("runs query").is_empty());
    drop(store);
    assert_eq!(service.run_execution.active_run_count(), 0);
    assert_eq!(service.run_execution.workspace_run_count(), 0);
}
