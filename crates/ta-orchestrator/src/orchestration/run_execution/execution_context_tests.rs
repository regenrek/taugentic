use super::*;
use crate::AppService;
use crate::orchestration::run_execution::test_support::*;
use ta_store::{
    CommitRepository, CommitRunTransition, EventLogRepository, ProjectionRepository,
    ScheduledWorkRepository, StoreSeedRepository,
};

fn scheduled_work_from_prepared_context(
    app: &AppService,
    execution: &RunExecutionService,
    session_id: &crate::SessionId,
    scheduled_work_id: &str,
    occurrence_id: &str,
    run_id: &RunId,
) -> (
    ta_protocol::wire::ScheduledWorkDefinition,
    ta_protocol::wire::ScheduledWorkOccurrence,
) {
    let selection = crate::orchestration::test_runtime_selection(app, "runtime-openai-safe");
    let route = app
        .agent_runtime
        .validate_agent_run_selection(&selection)
        .expect("scheduled route should validate")
        .route()
        .clone();
    let prepared = execution
        .prepare_execution_context(
            session_id,
            run_id,
            execution
                .agent_runtime
                .validate_agent_run_selection(&selection)
                .expect("scheduled profile should validate")
                .runtime_profile(),
            ExecutionContextRequest::workspace_write(),
        )
        .expect("template context should prepare");
    let definition = ta_protocol::wire::ScheduledWorkDefinition {
        id: ta_protocol::wire::ScheduledWorkId::new(scheduled_work_id).expect("scheduled work id"),
        session_id: session_id.clone(),
        objective: "Scheduled boundary test".to_string(),
        route,
        execution_request: ta_protocol::wire::ScheduledWorkExecutionRequest {
            workspace_id: prepared.execution_context.workspace_id.clone(),
            workspace_root: prepared.execution_context.workspace_root.clone(),
            repo_root: prepared.execution_context.workspace_root.clone(),
            artifact_root: prepared.execution_context.artifact_root.clone(),
            workspace_mode: ta_protocol::wire::WorkspaceMode::WorkspaceWrite,
            cleanup_policy: ta_protocol::wire::WorktreeCleanupPolicy::DeleteOnSuccess,
            planned_write_files: Vec::new(),
            workspace_scope: prepared.execution_context.workspace_scope,
            sandbox_profile: prepared.execution_context.sandbox_profile,
            permission_policy: prepared.execution_context.permission_policy,
            network_policy: prepared.execution_context.network_policy,
            env_policy: prepared.execution_context.env_policy,
        },
        due_at_ms: 10,
        attention_policy: ta_protocol::wire::ScheduledWorkAttentionPolicy::AttentionOnly,
    };
    let occurrence = ta_protocol::wire::ScheduledWorkOccurrence {
        id: ta_protocol::wire::ScheduledWorkOccurrenceId::new(occurrence_id)
            .expect("occurrence id"),
        scheduled_work_id: definition.id.clone(),
        due_at_ms: 10,
        state: ta_protocol::wire::ScheduledWorkOccurrenceState::Pending,
    };
    (definition, occurrence)
}

#[test]
fn scheduled_work_reserve_prepare_publish_creates_one_queued_run_without_dispatch() {
    let repo = init_dispatch_repo();
    let (runtime, dispatcher) = runtime_with_dispatch_plans([]);
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let session = open_session(&app, "Scheduled publish");
    let template_run_id = RunId::new("run-scheduled-template").expect("run id");
    let run_id = RunId::new("run-scheduled-publish").expect("run id");
    let (definition, occurrence) = scheduled_work_from_prepared_context(
        &app,
        &execution,
        &session.id,
        "schedule-publish",
        "occurrence-publish",
        &template_run_id,
    );
    execution
        .runtime
        .finish_scheduled_run(&session.id, &template_run_id, RunStatus::Cancelled);
    execution
        .store
        .lock()
        .expect("store")
        .create_scheduled_work(definition.clone(), occurrence.clone())
        .expect("scheduled work should persist");

    let published = execution
        .prepare_and_publish_scheduled_work(
            definition.id.clone(),
            occurrence.id.clone(),
            run_id.clone(),
        )
        .expect("scheduled work should publish");

    assert_eq!(published.id, run_id);
    assert_eq!(published.status, RunStatus::Queued);
    assert!(dispatcher.requests().is_empty());
    let store = execution.store.lock().expect("store");
    assert_eq!(
        store
            .runs()
            .expect("runs")
            .into_iter()
            .filter(|run| run.id == run_id)
            .count(),
        1
    );
    assert!(matches!(
        store
            .scheduled_work_occurrence(&occurrence.id)
            .expect("occurrence")
            .expect("stored occurrence")
            .state,
        ta_protocol::wire::ScheduledWorkOccurrenceState::Claimed { run_id: claimed }
            if claimed == run_id
    ));
}

#[test]
fn scheduled_preparation_cancellation_wins_publish_without_run() {
    let repo = init_dispatch_repo();
    let runtime = runtime_for_dispatch_repo(repo.path());
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let session = open_session(&app, "Scheduled cancellation");
    let template_run_id = RunId::new("run-scheduled-cancel-template").expect("run id");
    let run_id = RunId::new("run-scheduled-cancel").expect("run id");
    let (definition, occurrence) = scheduled_work_from_prepared_context(
        &app,
        &execution,
        &session.id,
        "schedule-cancel",
        "occurrence-cancel",
        &template_run_id,
    );
    execution
        .runtime
        .finish_scheduled_run(&session.id, &template_run_id, RunStatus::Cancelled);
    let mut store = execution.store.lock().expect("store");
    store
        .create_scheduled_work(definition.clone(), occurrence.clone())
        .expect("scheduled work");
    store
        .reserve_scheduled_work_occurrence(ta_store::ReserveScheduledWorkOccurrence {
            scheduled_work_id: definition.id.clone(),
            occurrence_id: occurrence.id.clone(),
            run_id: run_id.clone(),
        })
        .expect("reserve");
    let resource = execution
        .unpublished_scheduled_resource(
            &run_id,
            repo.path(),
            definition.execution_request.cleanup_policy,
        )
        .expect("resource identity");
    store
        .request_preparing_scheduled_work_cancellation(&occurrence.id, &run_id, resource)
        .expect("cancel intent");
    let run = ta_store::RunProjection {
        id: run_id.clone(),
        session_id: session.id.clone(),
        runtime_profile_id: definition.route.runtime_profile_id.clone(),
        objective: definition.objective.clone(),
        status: RunStatus::Queued,
        harness: definition.route.harness,
        source: ta_protocol::wire::RunSource::ScheduledWork {
            route: definition.route.clone(),
            scheduled_work_id: definition.id.clone(),
            occurrence_id: occurrence.id.clone(),
        },
        execution_context: ta_store::default_test_execution_context(),
        result: None,
        contract_violation: None,
        started_at_ms: None,
        ended_at_ms: None,
        last_event_seq: None,
        workspace_info: None,
        claimed_files: Vec::new(),
        conflict_summary: None,
    };
    assert!(
        store
            .publish_prepared_scheduled_work_occurrence(ta_store::ClaimScheduledWorkOccurrence {
                scheduled_work_id: definition.id,
                occurrence_id: occurrence.id,
                run
            })
            .is_err()
    );
    assert!(store.run(&run_id).expect("run lookup").is_none());
}

#[test]
fn scheduled_cleanup_required_retains_exact_resource_identity() {
    let repo = init_dispatch_repo();
    let runtime = runtime_for_dispatch_repo(repo.path());
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let session = open_session(&app, "Scheduled cleanup");
    let template_run_id = RunId::new("run-scheduled-cleanup-template").expect("run id");
    let run_id = RunId::new("run-scheduled-cleanup").expect("run id");
    let (definition, occurrence) = scheduled_work_from_prepared_context(
        &app,
        &execution,
        &session.id,
        "schedule-cleanup",
        "occurrence-cleanup",
        &template_run_id,
    );
    execution
        .runtime
        .finish_scheduled_run(&session.id, &template_run_id, RunStatus::Cancelled);
    let resource = execution
        .unpublished_scheduled_resource(
            &run_id,
            repo.path(),
            definition.execution_request.cleanup_policy,
        )
        .expect("resource identity");
    let mut store = execution.store.lock().expect("store");
    store
        .create_scheduled_work(definition.clone(), occurrence.clone())
        .expect("scheduled work");
    store
        .reserve_scheduled_work_occurrence(ta_store::ReserveScheduledWorkOccurrence {
            scheduled_work_id: definition.id,
            occurrence_id: occurrence.id.clone(),
            run_id: run_id.clone(),
        })
        .expect("reserve");
    let stored = store
        .finalize_preparing_scheduled_work_cleanup(
            &occurrence.id,
            &run_id,
            ta_protocol::wire::ScheduledWorkPreparationTerminal::Failed,
            resource.clone(),
            "preparation failed".to_string(),
            Err("exact cleanup failure".to_string()),
        )
        .expect("cleanup required should persist");
    assert!(matches!(
        stored.state,
        ta_protocol::wire::ScheduledWorkOccurrenceState::CleanupRequired { resource: retained, .. }
            if retained == resource
    ));
}

#[test]
fn scheduled_unavailable_frozen_repo_terminalizes_without_run_or_provider_dispatch() {
    let repo = init_dispatch_repo();
    let unavailable_repo = tempfile::tempdir().expect("unavailable repo root");
    let (runtime, dispatcher) = runtime_with_dispatch_plans([]);
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let session = open_session(&app, "Scheduled unavailable repo");
    let template_run_id = RunId::new("run-scheduled-unavailable-template").expect("run id");
    let run_id = RunId::new("run-scheduled-unavailable").expect("run id");
    let (mut definition, occurrence) = scheduled_work_from_prepared_context(
        &app,
        &execution,
        &session.id,
        "schedule-unavailable",
        "occurrence-unavailable",
        &template_run_id,
    );
    execution
        .runtime
        .finish_scheduled_run(&session.id, &template_run_id, RunStatus::Cancelled);

    set_default_test_workspace_root(&app, unavailable_repo.path());
    let unavailable = ta_protocol::wire::WorkspacePath::new(unavailable_repo.path())
        .expect("canonical unavailable root");
    definition.execution_request.workspace_root = unavailable.clone();
    definition.execution_request.repo_root = unavailable.clone();
    definition.execution_request.workspace_mode = ta_protocol::wire::WorkspaceMode::WorktreeWrite;

    execution
        .store
        .lock()
        .expect("store")
        .create_scheduled_work(definition.clone(), occurrence.clone())
        .expect("scheduled work should persist");

    assert!(
        execution
            .prepare_and_publish_scheduled_work(
                definition.id.clone(),
                occurrence.id.clone(),
                run_id.clone(),
            )
            .is_err()
    );
    assert!(dispatcher.requests().is_empty());

    let store = execution.store.lock().expect("store");
    assert!(store.run(&run_id).expect("run lookup").is_none());
    assert!(matches!(
        store
            .scheduled_work_occurrence(&occurrence.id)
            .expect("occurrence")
            .expect("stored occurrence")
            .state,
        ta_protocol::wire::ScheduledWorkOccurrenceState::CleanupRequired {
            run_id: retained_run_id,
            resource,
            intended_terminal: ta_protocol::wire::ScheduledWorkPreparationTerminal::Failed,
            ..
        } if retained_run_id == run_id
            && resource.parent_repo == unavailable.as_str()
            && resource.worktree_path
                == unavailable_repo
                    .path()
                    .join("target/taugentic-worktrees/run-scheduled-unavailable")
                    .to_string_lossy()
            && resource.branch == "ta/capsule-run-scheduled-unavailable"
            && resource.cleanup_policy
                == ta_protocol::wire::WorktreeCleanupPolicy::DeleteOnSuccess
    ));
}

#[test]
fn scheduled_boot_reconciles_preparing_before_scheduler_rehydration() {
    let repo = init_dispatch_repo();
    let runtime = runtime_for_dispatch_repo(repo.path());
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let session = open_session(&app, "Scheduled boot recovery");
    let template_run_id = RunId::new("run-scheduled-boot-template").expect("run id");
    let run_id = RunId::new("run-scheduled-boot").expect("run id");
    let (definition, occurrence) = scheduled_work_from_prepared_context(
        &app,
        &execution,
        &session.id,
        "schedule-boot",
        "occurrence-boot",
        &template_run_id,
    );
    execution
        .runtime
        .finish_scheduled_run(&session.id, &template_run_id, RunStatus::Cancelled);
    {
        let mut store = execution.store.lock().expect("store");
        store
            .create_scheduled_work(definition, occurrence.clone())
            .expect("scheduled work");
        store
            .reserve_scheduled_work_occurrence(ta_store::ReserveScheduledWorkOccurrence {
                scheduled_work_id: occurrence.scheduled_work_id.clone(),
                occurrence_id: occurrence.id.clone(),
                run_id: run_id.clone(),
            })
            .expect("reserve");
    }

    app.recover_on_boot()
        .expect("boot recovery should reconcile preparation");

    let store = execution.store.lock().expect("store");
    assert!(store.run(&run_id).expect("run lookup").is_none());
    assert!(matches!(
        store.scheduled_work_occurrence(&occurrence.id).expect("occurrence").expect("stored occurrence").state,
        ta_protocol::wire::ScheduledWorkOccurrenceState::PreparationFailed { run_id: failed, .. }
            if failed == run_id
    ));
}

#[test]
fn scheduled_resource_reattach_restores_worktree_and_file_claim_handles() {
    let repo = init_dispatch_repo();
    let runtime = runtime_for_dispatch_repo(repo.path());
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let session = open_session(&app, "Scheduled resource reattach");
    let run_id = RunId::new("run-scheduled-reattach").expect("run id");
    let selection = crate::orchestration::test_runtime_selection(&app, "runtime-openai-safe");
    let validated = app
        .agent_runtime
        .validate_agent_run_selection(&selection)
        .expect("route should validate");
    let prepared = execution
        .prepare_execution_context(
            &session.id,
            &run_id,
            validated.runtime_profile(),
            ExecutionContextRequest {
                workspace_mode: crate::WorkspaceMode::WorktreeWrite,
                cleanup_policy: crate::WorktreeCleanupPolicy::Keep,
                planned_write_files: vec!["src/lib.rs".to_string()],
            },
        )
        .expect("worktree should prepare");
    let run = ta_store::RunProjection {
        id: run_id.clone(),
        session_id: session.id.clone(),
        runtime_profile_id: validated.route().runtime_profile_id.clone(),
        objective: "Scheduled resource reattach".to_string(),
        status: RunStatus::Queued,
        harness: validated.route().harness,
        source: ta_protocol::wire::RunSource::ScheduledWork {
            route: validated.route().clone(),
            scheduled_work_id: ta_protocol::wire::ScheduledWorkId::new("schedule-reattach")
                .expect("scheduled work id"),
            occurrence_id: ta_protocol::wire::ScheduledWorkOccurrenceId::new("occurrence-reattach")
                .expect("occurrence id"),
        },
        execution_context: prepared.execution_context,
        result: None,
        contract_violation: None,
        started_at_ms: None,
        ended_at_ms: None,
        last_event_seq: None,
        workspace_info: prepared.workspace_info,
        claimed_files: prepared.claimed_files,
        conflict_summary: prepared.conflict_summary,
    };
    let store = execution.store.clone();
    store
        .lock()
        .expect("store")
        .save_run(run.clone())
        .expect("published run should persist");
    drop(execution);
    drop(app);

    let rehydrated_runtime = runtime_for_dispatch_repo(repo.path());
    let rehydrated = AppService::from_runtime(store, &rehydrated_runtime);
    rehydrated
        .run_execution
        .rehydrate_published_scheduled_resources(&run)
        .expect("resources should reattach");
    assert_eq!(rehydrated.run_execution.workspace_run_count(), 1);
    assert_eq!(rehydrated.run_execution.claim_count(), 1);
}

#[test]
fn execution_context_preparation_rejects_unverified_workspace() {
    let repo = init_dispatch_repo();
    let runtime = runtime_for_dispatch_repo(repo.path());
    let (app, execution) = app_and_execution_with_runtime(runtime);
    app.upsert_workspace(ta_store::test_workspace(
        ta_store::DEFAULT_TEST_WORKSPACE_ID,
        &repo.path().to_string_lossy(),
    ))
    .expect("unverified test workspace should update");
    let session = open_session(&app, "Unverified workspace");
    let runtime_profile = execution
        .runtime
        .runtime_profile(
            &crate::RuntimeProfileId::new("runtime-openai-safe").expect("runtime profile id"),
        )
        .expect("runtime profile");
    let run_id = RunId::new("run-unverified-workspace").expect("run id");

    let error = execution
        .prepare_execution_context(
            &session.id,
            &run_id,
            &runtime_profile,
            ExecutionContextRequest::workspace_write(),
        )
        .expect_err("unverified workspace should not resolve");

    assert!(matches!(
        error,
        RunExecutionError::WorkspaceTrustRequired(ref workspace_id)
            if workspace_id == ta_store::DEFAULT_TEST_WORKSPACE_ID
    ));
}

#[test]
fn execution_context_preparation_allocates_worktree_claims_and_cleans_success() {
    let repo = init_dispatch_repo();
    let runtime = runtime_for_dispatch_repo(repo.path());
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let session = open_session(&app, "Dispatch workspace");
    let run_id = RunId::new("run-worktree-dispatch").expect("run id");
    seed_dispatch_child(
        &execution,
        &session.id,
        &run_id,
        vec!["src/lib.rs"],
        crate::WorktreeCleanupPolicy::DeleteOnSuccess,
    );

    let stored = execution
        .store
        .lock()
        .expect("store")
        .run(&run_id)
        .expect("run lookup")
        .expect("run should exist");
    assert_eq!(stored.claimed_files, vec!["src/lib.rs".to_string()]);
    let worktree_path = std::path::PathBuf::from(
        stored
            .workspace_info
            .as_ref()
            .expect("workspace info")
            .path
            .clone(),
    );
    assert!(worktree_path.exists());
    assert!(stored.execution_context.effective_cwd.as_path().exists());

    execution
        .runtime
        .finish_scheduled_run(&session.id, &run_id, RunStatus::Completed);
    assert!(!worktree_path.exists());
}

#[test]
fn execution_context_preparation_emits_conflict_warning_for_overlapping_claims() {
    let repo = init_dispatch_repo();
    let runtime = runtime_for_dispatch_repo(repo.path());
    let (app, execution) = app_and_execution_with_runtime(runtime);
    set_default_test_workspace_root(&app, repo.path());
    let first_session = open_session(&app, "First dispatch");
    let second_session = open_session(&app, "Second dispatch");
    let first_run_id = RunId::new("run-conflict-a").expect("run id");
    let second_run_id = RunId::new("run-conflict-b").expect("run id");
    seed_dispatch_child(
        &execution,
        &first_session.id,
        &first_run_id,
        vec!["src/lib.rs"],
        crate::WorktreeCleanupPolicy::Manual,
    );
    seed_dispatch_child(
        &execution,
        &second_session.id,
        &second_run_id,
        vec!["src/lib.rs"],
        crate::WorktreeCleanupPolicy::Manual,
    );

    let store = execution.store.lock().expect("store");
    let second = store
        .run(&second_run_id)
        .expect("run lookup")
        .expect("run should exist");
    assert_eq!(
        second
            .conflict_summary
            .as_ref()
            .map(|summary| summary.warning_count),
        Some(1)
    );
    assert!(
        store
            .events_for_session(&second_session.id)
            .expect("events")
            .iter()
            .any(|record| {
                matches!(
                    &record.payload,
                    DaemonEvent::Conflict(crate::ConflictEvent::Warning { run_id, warning })
                        if run_id == &second_run_id
                            && warning.conflicts[0].holding_capsule == first_run_id
                )
            })
    );
}

fn runtime_for_dispatch_repo(repo: &std::path::Path) -> crate::RuntimeService {
    crate::RuntimeService::from_host_platform_with_paths(
        ta_host_platform::detect_current_platform(),
        crate::RuntimeExecutionPaths {
            artifact_root: repo.join("target/daemon-artifacts"),
        },
    )
}

fn seed_dispatch_child(
    execution: &RunExecutionService,
    session_id: &crate::SessionId,
    run_id: &RunId,
    planned_write_files: Vec<&str>,
    cleanup_policy: crate::WorktreeCleanupPolicy,
) {
    execution
        .runtime
        .schedule_run_start(session_id, run_id.clone())
        .expect("schedule should start");
    let runtime_profile = execution
        .runtime
        .runtime_profile(
            &crate::RuntimeProfileId::new("runtime-openai-safe").expect("runtime profile id"),
        )
        .expect("profile");
    let planned_write_files = planned_write_files
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let prepared_context = execution
        .prepare_execution_context(
            session_id,
            run_id,
            &runtime_profile,
            ExecutionContextRequest {
                workspace_mode: crate::WorkspaceMode::WorktreeWrite,
                cleanup_policy,
                planned_write_files: planned_write_files.clone(),
            },
        )
        .expect("execution context should prepare");
    let mut events = vec![DaemonEvent::Run(
        crate::RunEvent::active(run_id.clone(), RunStatus::Running, None, None, None)
            .expect("active status"),
    )];
    if let Some(warning) = prepared_context.conflict_warning.clone() {
        events.push(DaemonEvent::Conflict(crate::ConflictEvent::Warning {
            run_id: run_id.clone(),
            warning,
        }));
    }
    let mut store = execution.store.lock().expect("store");
    store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: runtime_profile.id,
                objective: "Dispatch child".to_string(),
                status: RunStatus::Running,
                harness: RunHarnessKind::Native,
                source: RunSource::NativeSubagent {
                    route: ta_store::default_test_run_source().route().clone(),
                    parent_run_id: RunId::new("run-parent-dispatch").expect("parent id"),
                    parent_turn_id: ta_protocol::wire::AgentStreamTurnId::new("turn-dispatch")
                        .expect("turn id"),
                    output_contract: None,
                    model_id: None,
                    recipe_id: None,
                    workspace_scope: crate::WorkspaceMode::WorktreeWrite,
                    cleanup_policy,
                    planned_write_files,
                },
                execution_context: prepared_context.execution_context,
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: prepared_context.workspace_info,
                claimed_files: prepared_context.claimed_files,
                conflict_summary: prepared_context.conflict_summary,
            },
            user_turn: ta_store::UserTurnCommit::NoUserTurn,
            events,
            occurred_at_ms: current_time_ms(),
            auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
        })
        .expect("run should seed");
    execution
        .runtime
        .claim_live_run(run_id.clone(), session_id.clone());
}
