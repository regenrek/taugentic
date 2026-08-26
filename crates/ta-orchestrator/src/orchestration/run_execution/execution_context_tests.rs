use super::*;
use crate::orchestration::run_execution::test_support::*;
use ta_store::{CommitRepository, CommitRunTransition, EventLogRepository, ProjectionRepository};

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
    let mut events = vec![DaemonEvent::Run(crate::RunEvent {
        run_id: run_id.clone(),
        status: RunStatus::Running,
        detail: "seed dispatch child".to_string(),
        output_contract: None,
        recipe_id: None,
        result: None,
    })];
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
            events,
            occurred_at_ms: current_time_ms(),
        })
        .expect("run should seed");
    execution
        .runtime
        .claim_live_run(run_id.clone(), session_id.clone());
}
