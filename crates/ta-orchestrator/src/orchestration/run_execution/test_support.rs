use std::sync::{Arc, Mutex};

use crate::{
    RunId, SessionId,
    orchestration::{AppService, RunExecutionService},
};
use ta_protocol::wire::{ApprovalActor, ApprovalResolution};
use ta_store::InMemoryStore;
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
    let app = AppService::from_runtime(store, &runtime);
    let execution = app.run_execution.clone();
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

pub(super) fn init_dispatch_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("temp repo");
    dispatch_git(repo.path(), ["init"]);
    dispatch_git(repo.path(), ["config", "user.email", "agent@example.test"]);
    dispatch_git(repo.path(), ["config", "user.name", "Agent Test"]);
    std::fs::write(repo.path().join(".gitignore"), "target/\n").expect("gitignore");
    std::fs::create_dir_all(repo.path().join("src")).expect("src dir");
    std::fs::write(repo.path().join("src/lib.rs"), "pub fn fixture() {}\n").expect("fixture");
    dispatch_git(repo.path(), ["add", "."]);
    dispatch_git(repo.path(), ["commit", "-m", "initial"]);
    repo
}

fn dispatch_git<const N: usize>(repo: &std::path::Path, args: [&str; N]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git should run");
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn ensure_running_run(
    app: &AppService<InMemoryStore>,
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &SessionId,
    objective: &str,
) -> RunSummary {
    ensure_running_run_with_profile(app, execution, session_id, objective, "runtime-openai-safe")
}

pub(super) fn ensure_running_run_with_profile(
    app: &AppService<InMemoryStore>,
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &SessionId,
    objective: &str,
    runtime_profile_id: &str,
) -> RunSummary {
    let selection = validated_runtime_selection(app, runtime_profile_id);
    execution
        .seed_running_run_for_tests(session_id.clone(), objective.to_string(), selection)
        .expect("seeded running run should persist")
        .run
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

pub(super) fn start_run_command(
    app: &AppService<InMemoryStore>,
    objective: &str,
    runtime_profile_id: &str,
) -> crate::StartRunCommand {
    crate::StartRunCommand::new(
        objective,
        crate::orchestration::test_runtime_selection(app, runtime_profile_id),
    )
}

pub(super) fn validated_runtime_selection(
    app: &AppService<InMemoryStore>,
    runtime_profile_id: &str,
) -> crate::orchestration::ValidatedRunSelection {
    let selection = crate::orchestration::test_runtime_selection(app, runtime_profile_id);
    app.agent_runtime
        .validate_run_selection(&selection)
        .expect("explicit runtime selection should validate")
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
