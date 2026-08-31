use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{Receiver, SyncSender},
    },
};

use crate::{
    RunId, SessionId,
    orchestration::{AppService, RunExecutionService},
};
use ta_protocol::wire::{AgentTurnRow, ApprovalActor, ApprovalResolution};
use ta_store::{EventLogRepository, InMemoryStore, SessionAgentTurnsPageQuery};
use taugentic_agent::{ExecutionHandle, ExecutionRequest, ExecutionSink};

use super::*;

pub(crate) enum DispatchPlan {
    Succeed(Arc<dyn ExecutionHandle>),
    Fail(String),
    GateThenFail {
        entered: SyncSender<()>,
        release: Receiver<()>,
        message: String,
    },
}

#[derive(Clone)]
pub(crate) struct FixtureDispatcher {
    plans: Arc<Mutex<VecDeque<DispatchPlan>>>,
    requests: Arc<Mutex<Vec<ExecutionRequest>>>,
}

impl FixtureDispatcher {
    pub(crate) fn new(plans: impl IntoIterator<Item = DispatchPlan>) -> Self {
        Self {
            plans: Arc::new(Mutex::new(plans.into_iter().collect())),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn requests(&self) -> Vec<ExecutionRequest> {
        self.requests
            .lock()
            .expect("fixture dispatch requests should not be poisoned")
            .clone()
    }
}

impl crate::orchestration::service::RunExecutionDispatcher for FixtureDispatcher {
    fn dispatch(
        &self,
        request: ExecutionRequest,
        _sink: Arc<dyn ExecutionSink>,
    ) -> Result<Arc<dyn ExecutionHandle>, crate::AgentRuntimeServiceError> {
        self.requests
            .lock()
            .expect("fixture dispatch requests should not be poisoned")
            .push(request);
        let plan = self
            .plans
            .lock()
            .expect("fixture dispatch plans should not be poisoned")
            .pop_front();
        match plan {
            Some(DispatchPlan::Succeed(handle)) => Ok(handle),
            Some(DispatchPlan::Fail(message)) => Err(
                crate::AgentRuntimeServiceError::ProviderExecutionFailed(message),
            ),
            Some(DispatchPlan::GateThenFail {
                entered,
                release,
                message,
            }) => {
                entered.send(()).expect("test must await gated dispatch");
                release.recv().expect("test must release gated dispatch");
                Err(crate::AgentRuntimeServiceError::ProviderExecutionFailed(
                    message,
                ))
            }
            None => Err(crate::AgentRuntimeServiceError::ProviderExecutionFailed(
                "fixture dispatcher has no plan".to_string(),
            )),
        }
    }
}

pub(super) fn runtime_with_dispatch_plans(
    plans: impl IntoIterator<Item = DispatchPlan>,
) -> (crate::RuntimeService, FixtureDispatcher) {
    let dispatcher = FixtureDispatcher::new(plans);
    let runtime = crate::RuntimeService::from_host_platform_with_paths_and_dispatcher(
        ta_host_platform::detect_current_platform(),
        crate::RuntimeExecutionPaths {
            artifact_root: PathBuf::from("/tmp/taugentic-fixture-artifacts"),
        },
        Arc::new(dispatcher.clone()),
    );
    (runtime, dispatcher)
}

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

pub(super) fn start_production_shaped_running_run(
    app: &AppService<InMemoryStore>,
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &SessionId,
    objective: &str,
) -> RunId {
    let started = execution
        .start_run(
            session_id.clone(),
            start_run_command(app, objective, "runtime-openai-safe"),
        )
        .expect("production-shaped parent should start");
    let approval_id = started
        .requested_approval_id()
        .expect("safe production-shaped parent requires approval");
    execution
        .decide_approval(
            session_id.clone(),
            approval_actor(),
            crate::DaemonApprovalDecideParams {
                approval_id,
                decision: ta_protocol::wire::ApprovalDecision::Approved,
                commentary: None,
            },
        )
        .expect("production-shaped parent approval should dispatch");
    started.run.id
}

pub(super) fn durable_user_turns_for_run(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &SessionId,
    run_id: &RunId,
) -> Vec<String> {
    execution
        .store
        .lock()
        .expect("store")
        .session_agent_turns_page(&SessionAgentTurnsPageQuery {
            session_id: session_id.clone(),
            before_sequence: None,
            limit: 100,
        })
        .expect("turn rows")
        .rows
        .into_iter()
        .filter_map(|row| match row {
            AgentTurnRow::User(user) if user.run_id == *run_id => Some(user.text),
            _ => None,
        })
        .collect()
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
        generation: execution
            .runtime
            .live_execution_for(run_id)
            .filter(|live_execution| live_execution.session_id == *session_id)
            .expect("live execution")
            .generation,
    }
}

pub(super) fn start_native_child_run_for_tests(
    execution: &RunExecutionService<InMemoryStore>,
    session_id: &SessionId,
    request: taugentic_agent::NativeChildRunRequest,
) -> Result<taugentic_agent::NativeChildRunResult, RunExecutionError> {
    let generation = execution
        .runtime
        .live_execution_for(&request.parent_run_id)
        .filter(|live_execution| live_execution.session_id == *session_id)
        .ok_or_else(|| {
            RunExecutionError::RunNotLiveOwned(request.parent_run_id.as_str().to_string())
        })?
        .generation;
    execution.start_native_child_run_from_generation(session_id.clone(), request, generation)
}

pub(crate) struct NoopExecutionHandle;

impl ExecutionHandle for NoopExecutionHandle {
    fn cancel(&self) -> Result<(), taugentic_agent::ExecutionError> {
        Ok(())
    }
}

pub(super) struct CountingExecutionHandle(pub(super) Arc<AtomicUsize>);

impl ExecutionHandle for CountingExecutionHandle {
    fn cancel(&self) -> Result<(), taugentic_agent::ExecutionError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

pub(super) fn attach_counting_handle(
    execution: &RunExecutionService<InMemoryStore>,
    run_id: &RunId,
) -> Arc<AtomicUsize> {
    let count = Arc::new(AtomicUsize::new(0));
    execution
        .runtime
        .attach_live_run_handle_for_tests(run_id, Arc::new(CountingExecutionHandle(count.clone())))
        .expect("counting handle should attach");
    count
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

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, thread};

    use ta_protocol::wire::{AgentRuntimeStrategyId, RuntimeProfileId};
    use taugentic_agent::{AgentExecutionHarness, ExecutionError};

    use super::*;

    struct Sink;

    impl ExecutionSink for Sink {
        fn push_stream(&self, _: taugentic_agent::StreamEmission) -> Result<(), ExecutionError> {
            Ok(())
        }
        fn record_token_usage(
            &self,
            _: ta_provider_llm::client::LlmTokenUsage,
        ) -> Result<(), ExecutionError> {
            Ok(())
        }
        fn push_activity(&self, _: &str) -> Result<(), ExecutionError> {
            Ok(())
        }
        fn push_provider_session_id(&self, _: String) -> Result<(), ExecutionError> {
            Ok(())
        }
        fn request_approval(
            &self,
            _: ta_protocol::wire::ApprovalRequest,
        ) -> Result<(), ExecutionError> {
            Ok(())
        }
        fn resolve_approval(
            &self,
            _: ta_protocol::wire::ApprovalResolution,
        ) -> Result<(), ExecutionError> {
            Ok(())
        }
        fn record_artifact(
            &self,
            _: ta_protocol::wire::ArtifactKind,
            _: &str,
        ) -> Result<(), ExecutionError> {
            Ok(())
        }
        fn record_image_artifact(
            &self,
            _: ta_protocol::wire::AgentStreamTurnId,
            _: ta_protocol::wire::AgentStreamItemId,
            _: &str,
        ) -> Result<(), ExecutionError> {
            Ok(())
        }
        fn complete(&self, _: &str) -> Result<(), ExecutionError> {
            Ok(())
        }
        fn fail(&self, _: ExecutionError) -> Result<(), ExecutionError> {
            Ok(())
        }
    }

    fn request() -> ExecutionRequest {
        ExecutionRequest {
            session_id: SessionId::new("session-fixture").expect("session id"),
            run_id: RunId::new("run-fixture").expect("run id"),
            runtime_profile_id: RuntimeProfileId::new("runtime-fixture").expect("profile id"),
            provider_id: AgentRuntimeStrategyId::new("provider-fixture").expect("provider id"),
            execution_harness: AgentExecutionHarness::NativeLoop,
            system_prompt: None,
            objective: "fixture dispatch".to_string(),
            model_id: None,
            auth_profile_id: None,
            resume_provider_session_id: None,
            runtime_extensions: Vec::new(),
            execution_context: Arc::new(ta_store::default_test_execution_context()),
            native_history: None,
            output_contract: None,
            subagent_recipes: Vec::new(),
            attachments: Vec::new(),
            dsh_tool_approval_manifest: BTreeMap::new(),
        }
    }

    #[test]
    fn fixture_dispatcher_releases_plan_queue_before_gate_wait() {
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        let dispatcher = FixtureDispatcher::new([
            DispatchPlan::GateThenFail {
                entered: entered_tx,
                release: release_rx,
                message: "first plan fails after release".to_string(),
            },
            DispatchPlan::Succeed(Arc::new(NoopExecutionHandle)),
        ]);
        let first = dispatcher.clone();
        let first_call = thread::spawn(move || {
            crate::orchestration::service::RunExecutionDispatcher::dispatch(
                &first,
                request(),
                Arc::new(Sink),
            )
        });
        entered_rx.recv().expect("first dispatch must be gated");

        crate::orchestration::service::RunExecutionDispatcher::dispatch(
            &dispatcher,
            request(),
            Arc::new(Sink),
        )
        .expect("second plan must dispatch while first remains gated");
        release_tx.send(()).expect("release first plan");
        assert!(
            first_call
                .join()
                .expect("first dispatch must return")
                .is_err()
        );
        assert_eq!(dispatcher.requests().len(), 2);
    }
}
