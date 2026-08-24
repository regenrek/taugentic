use std::sync::{Arc, Mutex};

use crate::{
    RunId, SessionId,
    orchestration::{AppService, RunExecutionService},
};
use ta_protocol::wire::{ApprovalActor, ApprovalResolution, RunHarnessKind, RunSource, RunStatus};
use ta_store::{CommitRepository, CommitRunTransition, InMemoryStore, RunProjection};
use taugentic_agent::ExecutionHandle;

use super::*;

pub(super) const TEST_CLIENT_NAME: &str = "run-execution-tests";
pub(super) const TEST_OWNER_PRINCIPAL_ID: &str = "run-execution-owner-credential-hash";

pub(super) fn app_and_execution_with_runtime(
    runtime: crate::RuntimeService,
) -> (
    AppService<InMemoryStore>,
    RunExecutionService<InMemoryStore>,
) {
    let store = Arc::new(Mutex::new(InMemoryStore::current()));
    let execution_runtime = runtime.run_execution_runtime();
    let recipe_registry =
        Arc::new(crate::RecipeRegistry::load_builtin().expect("built-in recipes should load"));
    let execution =
        RunExecutionService::new(store.clone(), execution_runtime.clone(), recipe_registry);
    let app = AppService::from_runtime(store, &runtime);
    app.upsert_workspace(ta_store::default_test_workspace())
        .expect("seed default test workspace");
    (app, execution)
}

pub(super) fn set_default_test_workspace_root(
    app: &AppService<InMemoryStore>,
    root: &std::path::Path,
) {
    app.upsert_workspace(ta_store::confirmed_test_workspace(
        ta_store::DEFAULT_TEST_WORKSPACE_ID,
        &root.to_string_lossy(),
    ))
    .expect("test workspace should update");
}

pub(super) fn ensure_running_run(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &SessionId,
    objective: &str,
) -> RunSummary {
    let run_id = RunId::new(format!("run-{}", uuid::Uuid::new_v4().simple())).expect("run id");
    let runtime_profile = execution
        .runtime
        .selected_runtime_profile()
        .expect("selected runtime profile should exist");
    {
        let mut store = execution
            .store
            .lock()
            .expect("app store should not be poisoned");
        store
            .commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: RunProjection {
                    id: run_id.clone(),
                    session_id: session_id.clone(),
                    runtime_profile_id: runtime_profile.id.clone(),
                    objective: objective.to_string(),
                    status: RunStatus::Running,
                    harness: RunHarnessKind::Native,
                    source: RunSource::default(),
                    execution_context: ta_store::default_test_execution_context(),
                    result: None,
                    contract_violation: None,
                    started_at_ms: None,
                    ended_at_ms: None,
                    last_event_seq: None,
                    workspace_info: None,
                    claimed_files: Vec::new(),
                    conflict_summary: None,
                },
                events: vec![DaemonEvent::Run(crate::RunEvent {
                    run_id: run_id.clone(),
                    status: RunStatus::Running,
                    detail: "Seeded live run for owner-layer proof".to_string(),
                    output_contract: None,
                    recipe_id: None,
                    result: None,
                })],
                occurred_at_ms: current_time_ms(),
            })
            .expect("seeded running run should persist");
    }
    execution
        .runtime
        .claim_live_run(run_id.clone(), session_id.clone());
    RunSummary {
        id: run_id,
        runtime_profile_id: runtime_profile.id,
        objective: objective.to_string(),
        status: RunStatus::Running,
    }
}

pub(super) fn provider_sink(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &SessionId,
    run_id: &RunId,
) -> ProviderRunExecutionSink<InMemoryStore> {
    ProviderRunExecutionSink {
        service: execution.clone(),
        session_id: session_id.clone(),
        run_id: run_id.clone(),
    }
}

pub(super) struct NoopExecutionHandle;

impl ExecutionHandle for NoopExecutionHandle {
    fn cancel(&self) -> Result<(), taugentic_agent::ExecutionError> {
        Ok(())
    }
}

pub(super) struct RecordingExecutionHandle {
    resolved: Arc<Mutex<Vec<ApprovalResolution>>>,
}

impl ExecutionHandle for RecordingExecutionHandle {
    fn cancel(&self) -> Result<(), taugentic_agent::ExecutionError> {
        Ok(())
    }

    fn resolve_approval(
        &self,
        resolution: ApprovalResolution,
    ) -> Result<(), taugentic_agent::ExecutionError> {
        self.resolved
            .lock()
            .expect("recorded approvals should not be poisoned")
            .push(resolution);
        Ok(())
    }
}

pub(super) fn attach_noop_handle(execution: &RunExecutionService<InMemoryStore>, run_id: &RunId) {
    execution
        .runtime
        .attach_live_run_handle_for_tests(run_id, Arc::new(NoopExecutionHandle))
        .expect("noop handle should attach");
}

pub(super) fn attach_recording_handle(
    execution: &RunExecutionService<InMemoryStore>,
    run_id: &RunId,
) -> Arc<Mutex<Vec<ApprovalResolution>>> {
    let resolved = Arc::new(Mutex::new(Vec::new()));
    execution
        .runtime
        .attach_live_run_handle_for_tests(
            run_id,
            Arc::new(RecordingExecutionHandle {
                resolved: resolved.clone(),
            }),
        )
        .expect("recording handle should attach");
    resolved
}

pub(super) fn approval_actor() -> ApprovalActor {
    ApprovalActor::new(TEST_OWNER_PRINCIPAL_ID).expect("approval actor")
}

pub(super) fn select_runtime_profile(app: &AppService<InMemoryStore>, runtime_profile_id: &str) {
    app.select_agent_runtime_profile(&crate::DaemonAgentRuntimeSelectProfileParams {
        runtime_profile_id: crate::RuntimeProfileId::new(runtime_profile_id)
            .expect("runtime profile id"),
    })
    .expect("runtime profile should select");
}

pub(super) fn open_session(app: &AppService<InMemoryStore>, title: &str) -> crate::SessionSummary {
    app.open_session(
        TEST_CLIENT_NAME,
        TEST_OWNER_PRINCIPAL_ID,
        &crate::orchestration::OpenSessionRequest {
            title: title.to_string(),
            workspace_id: ta_store::default_test_workspace_id(),
        },
    )
    .expect("session should open")
    .session
}
