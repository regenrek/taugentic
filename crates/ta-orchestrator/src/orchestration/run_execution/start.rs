use ta_policy::Operation;
use ta_protocol::wire::StartRunCommand;
use ta_store::CommitRunTransition;
use uuid::Uuid;

use super::*;
use crate::{
    DelegateRecipeResolutionRequest, ResolvedDelegateRecipeRequest, resolve_delegate_recipe,
};

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn start_run(
        &self,
        session_id: crate::SessionId,
        command: StartRunCommand,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let resolved_command = self.resolve_start_run_command(command)?;
        let objective = resolved_command.objective.trim();
        if objective.is_empty() {
            return Err(RunExecutionError::EmptyRunObjective);
        }
        {
            let store = self.store.lock().expect("app store should not be poisoned");
            if store.session(&session_id)?.is_none() {
                return Err(RunExecutionError::SessionNotFound(
                    session_id.as_str().to_string(),
                ));
            }
        }

        let run_id = crate::RunId::new(format!("run-{}", Uuid::new_v4().simple()))
            .expect("generated run id should be valid");
        let disposition = self
            .runtime
            .schedule_run_start(&session_id, run_id.clone())
            .map_err(|error| match error {
                crate::RunSchedulerError::QueueFull(session_id) => {
                    RunExecutionError::RunQueueFull(session_id)
                }
            })?;
        let operation = Operation::new(ApprovalScope::ProcessExec, "execute run");
        let decision = self
            .runtime
            .evaluate_operation(&operation)
            .map_err(map_agent_runtime_error)?;
        let runtime_profile = self
            .runtime
            .selected_runtime_profile()
            .map_err(map_agent_runtime_error)?;
        let harness = self
            .runtime
            .execution_harness_for_runtime_profile(&runtime_profile)
            .map_err(map_agent_runtime_error)?;

        let (mut run, mut events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let (status, events) = match disposition {
                crate::RunScheduleDisposition::StartNow => build_start_transition(
                    run_id.clone(),
                    decision,
                    resolved_command.recipe_id.clone(),
                ),
                crate::RunScheduleDisposition::Queued { position } => build_queue_transition(
                    run_id.clone(),
                    position,
                    resolved_command.recipe_id.clone(),
                ),
            };
            let run = RunProjection {
                id: run_id,
                session_id: session_id.clone(),
                runtime_profile_id: runtime_profile.id.clone(),
                objective: objective.to_string(),
                status,
                harness: run_harness_kind(&harness),
                source: RunSource::User {
                    output_contract: resolved_command.output_contract,
                    model_id: resolved_command.model_id.clone(),
                    sandbox_profile: resolved_command.sandbox_profile.clone(),
                    recipe_id: resolved_command.recipe_id.clone(),
                },
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: None,
                claimed_files: Vec::new(),
                conflict_summary: None,
            };
            let committed = store.commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: run.clone(),
                events,
                occurred_at_ms: current_time_ms(),
            })?;
            if committed.run.status == RunStatus::Running {
                self.runtime
                    .claim_live_run(committed.run.id.clone(), session_id.clone());
            }
            (committed.run, committed.events)
        };
        if run.status == RunStatus::Running {
            let start_result = self.start_provider_execution(
                &session_id,
                &run.id,
                &run.objective,
                &runtime_profile,
                execution_overrides_for_run(&run),
            );
            let latest_run = self.load_run_projection(&run.id)?;
            if let Err(error) = start_result
                && latest_run.status == RunStatus::Running
            {
                let (failed_run, failed_events) = self.fail_live_run_without_publish(
                    session_id.clone(),
                    &latest_run.id,
                    error.to_string(),
                )?;
                run = failed_run;
                events.extend(failed_events);
            } else if latest_run.status != RunStatus::Cancelled {
                run = latest_run;
            }
        }
        if matches!(run.status, RunStatus::Failed) {
            events.extend(self.advance_ready_queue(&session_id, &run.id, RunStatus::Failed)?);
        }

        let run = project_run_summary(run);
        Ok(RunMutationResult { run, events })
    }

    fn resolve_start_run_command(
        &self,
        command: StartRunCommand,
    ) -> Result<ResolvedDelegateRecipeRequest, RunExecutionError> {
        resolve_delegate_recipe(
            &self.recipe_registry,
            DelegateRecipeResolutionRequest {
                objective: command.objective,
                output_contract: None,
                model_id: command.model_id,
                sandbox_profile: command.sandbox_profile,
                recipe_id: command.recipe_id,
            },
        )
        .map_err(map_recipe_resolution_error)
    }

    pub(super) fn start_provider_execution(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
        objective: &str,
        runtime_profile: &crate::RuntimeProfileSummary,
        overrides: ExecutionRequestOverrides,
    ) -> Result<(), RunExecutionError> {
        self.enforce_budget_before_dispatch(session_id, run_id)?;
        let preflight = self.dispatch_preflight(session_id, run_id)?;
        let fork_initial_state = self.fork_initial_state_for_run(session_id, run_id)?;
        let output_contract = self
            .load_run_projection(run_id)
            .map(|run| output_contract_for_run(&run))?;
        self.runtime
            .start_provider_run(
                crate::ProviderRunStart {
                    runtime_profile,
                    session_id,
                    run_id,
                    objective,
                    working_directory: preflight.working_directory,
                    fork_initial_state,
                    output_contract,
                    model_id: overrides.model_id.as_ref(),
                    sandbox_profile: overrides.sandbox_profile.as_deref(),
                    subagent_recipes: self
                        .recipe_registry
                        .recipes()
                        .into_iter()
                        .cloned()
                        .collect(),
                },
                Arc::new(ProviderRunExecutionSink {
                    service: self.clone(),
                    session_id: session_id.clone(),
                    run_id: run_id.clone(),
                }),
            )
            .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))
    }

    pub(super) fn dispatch_preflight(
        &self,
        session_id: &crate::SessionId,
        run_id: &RunId,
    ) -> Result<ProviderDispatchPreflight, RunExecutionError> {
        let existing_run = self.load_run_projection(run_id)?;
        let (workspace_scope, cleanup_policy, planned_write_files) =
            dispatch_workspace_request(&existing_run);
        let dispatch = self
            .runtime
            .prepare_dispatch_workspace(
                run_id,
                workspace_scope,
                cleanup_policy,
                &planned_write_files,
            )
            .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;

        if dispatch.worktree_info.is_none()
            && dispatch.claimed_files.is_empty()
            && dispatch.conflict_warning.is_none()
        {
            return Ok(ProviderDispatchPreflight {
                working_directory: dispatch.working_directory,
            });
        }

        let run = RunProjection {
            workspace_info: dispatch.worktree_info.clone(),
            claimed_files: dispatch.claimed_files.clone(),
            conflict_summary: dispatch
                .conflict_warning
                .as_ref()
                .map(conflict_summary_for_warning),
            ..existing_run
        };
        let mut events = vec![DaemonEvent::Run(crate::RunEvent {
            run_id: run.id.clone(),
            status: run.status,
            detail: "Dispatch workspace prepared".to_string(),
            output_contract: None,
            recipe_id: recipe_id_for_run(&run),
            result: None,
        })];
        if let Some(warning) = dispatch.conflict_warning {
            events.push(DaemonEvent::Conflict(crate::ConflictEvent::Warning {
                run_id: run.id.clone(),
                warning,
            }));
        }
        let committed = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            store.commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run,
                events,
                occurred_at_ms: current_time_ms(),
            })?
        };
        self.publish_records(&committed.events);
        Ok(ProviderDispatchPreflight {
            working_directory: dispatch.working_directory,
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct ProviderDispatchPreflight {
    pub working_directory: std::path::PathBuf,
}

fn dispatch_workspace_request(
    run: &RunProjection,
) -> (
    crate::WorkspaceMode,
    crate::WorktreeCleanupPolicy,
    Vec<String>,
) {
    match &run.source {
        RunSource::NativeSubagent {
            workspace_scope,
            cleanup_policy,
            planned_write_files,
            ..
        } => (
            *workspace_scope,
            *cleanup_policy,
            planned_write_files.clone(),
        ),
        RunSource::User { .. } | RunSource::Forked { .. } => (
            crate::WorkspaceMode::WorkspaceWrite,
            crate::WorktreeCleanupPolicy::DeleteOnSuccess,
            Vec::new(),
        ),
    }
}

fn conflict_summary_for_warning(
    warning: &ta_protocol::wire::ConflictWarning,
) -> crate::ConflictSummary {
    let mut files = warning
        .conflicts
        .iter()
        .map(|conflict| conflict.file.clone())
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    crate::ConflictSummary {
        warning_count: warning.conflicts.len() as u32,
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::run_execution::test_support::*;
    use crate::{SessionId, StartRunCommand};
    use ta_protocol::wire::{OutputContractKind, RunSource, RunStatus};
    use ta_store::{CommitRepository, EventLogRepository, ProjectionRepository};

    #[test]
    fn start_run_rejects_blank_objective() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                },
            )
            .expect("session should open");

        let error = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "   ".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect_err("blank objective must fail");

        assert!(matches!(error, RunExecutionError::EmptyRunObjective));
    }

    #[test]
    fn start_run_rejects_unknown_session() {
        let runtime = crate::RuntimeService::bootstrap();
        let (_, execution) = app_and_execution_with_runtime(runtime);

        let error = execution
            .start_run(
                SessionId::new("session-missing").expect("session id"),
                StartRunCommand {
                    objective: "Ship app server hard cut".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect_err("missing session must fail");

        assert!(matches!(
            error,
            RunExecutionError::SessionNotFound(ref session_id) if session_id == "session-missing"
        ));
    }

    #[test]
    fn start_run_with_require_approval_mode_waits_for_approval() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                },
            )
            .expect("session should open");

        select_runtime_profile(&app, "runtime-codex-safe");
        let started = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "Ship policy gated run".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("run should start");

        assert_eq!(started.run.status, RunStatus::WaitingForApproval);
        assert!(started.requested_approval_id().is_some());
    }

    #[test]
    fn start_run_with_allow_mode_runs_without_approval() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                },
            )
            .expect("session should open");

        select_runtime_profile(&app, "runtime-codex-allow");
        let started = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "Ship policy allow run".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("run should start");

        assert_ne!(started.run.status, RunStatus::WaitingForApproval);
        assert!(started.requested_approval_id().is_none());
    }

    #[test]
    fn start_run_resolves_recipe_before_provider_execution() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                },
            )
            .expect("session should open");

        select_runtime_profile(&app, "runtime-codex-allow");
        let started = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "Find the failing login redirect".to_string(),
                    recipe_id: Some("debug-agent".to_string()),
                    model_id: None,
                    sandbox_profile: Some("read-only".to_string()),
                },
            )
            .expect("recipe-backed run should start");
        let run = execution
            .load_run_projection(&started.run.id)
            .expect("started run should be durable");

        assert!(run.objective.contains("Find the failing login redirect"));
        assert_eq!(
            output_contract_for_run(&run),
            Some(OutputContractKind::Debug)
        );
        assert_eq!(recipe_id_for_run(&run).as_deref(), Some("debug-agent"));
        assert_eq!(
            execution_overrides_for_run(&run).sandbox_profile.as_deref(),
            Some("read-only")
        );
        assert!(matches!(
            run.source,
            RunSource::User {
                recipe_id: Some(ref recipe_id),
                ..
            } if recipe_id == "debug-agent"
        ));
    }

    #[test]
    fn start_run_with_deny_mode_fails_immediately() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                },
            )
            .expect("session should open");

        select_runtime_profile(&app, "runtime-codex-deny");
        let started = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "Ship policy denied run".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("run should start");

        assert_eq!(started.run.status, RunStatus::Failed);
        assert!(started.requested_approval_id().is_none());
    }

    #[test]
    fn patching_selected_profile_updates_live_run_policy() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                },
            )
            .expect("session should open");

        let snapshot = app
            .patch_agent_runtime_profile(&crate::DaemonAgentRuntimePatchProfileParams {
                runtime_profile_id: crate::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                patch: crate::RuntimeProfilePatch {
                    policy_mode: Some(crate::RuntimePolicyMode::Allow),
                    ..Default::default()
                },
            })
            .expect("runtime profile patch should succeed");
        let selected_profile = snapshot
            .runtime_profiles
            .iter()
            .find(|profile| profile.id.as_str() == "runtime-codex-safe")
            .expect("selected runtime profile should exist");

        let started = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "Ship patched policy run".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("run should start");

        assert_eq!(
            selected_profile.policy_mode,
            crate::RuntimePolicyMode::Allow
        );
        assert_ne!(started.run.status, RunStatus::WaitingForApproval);
        assert!(started.requested_approval_id().is_none());
    }

    #[test]
    fn start_run_queues_when_session_already_has_active_run() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                },
            )
            .expect("session should open");

        let first = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "Ship queue owner".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("first run should start");
        let second = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "Ship follow-up queue item".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("second run should queue");

        let runs = app.list_runs(&session.id).expect("runs should list");

        assert!(matches!(
            first.run.status,
            RunStatus::Running | RunStatus::WaitingForApproval
        ));
        assert_eq!(second.run.status, RunStatus::Queued);
        assert!(second.requested_approval_id().is_none());
        assert!(
            runs.iter()
                .any(|run| run.id == second.run.id && run.status == RunStatus::Queued)
        );
    }

    #[test]
    fn dispatch_preflight_allocates_worktree_claims_files_and_cleans_success() {
        let repo = init_dispatch_repo();
        let runtime = runtime_for_dispatch_repo(repo.path());
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Dispatch workspace");
        let run_id = RunId::new("run-worktree-dispatch").expect("run id");
        seed_dispatch_child(
            &execution,
            &session.id,
            &run_id,
            vec!["src/lib.rs"],
            crate::WorktreeCleanupPolicy::DeleteOnSuccess,
        );

        let preflight = execution
            .dispatch_preflight(&session.id, &run_id)
            .expect("dispatch preflight should succeed");
        let worktree_path = preflight.working_directory;
        assert!(worktree_path.exists());

        let stored = execution
            .store
            .lock()
            .expect("store")
            .run(&run_id)
            .expect("run lookup")
            .expect("run should exist");
        assert_eq!(stored.claimed_files, vec!["src/lib.rs".to_string()]);
        assert!(stored.workspace_info.is_some());

        execution
            .runtime
            .finish_scheduled_run(&session.id, &run_id, RunStatus::Completed);
        assert!(!worktree_path.exists());
    }

    #[test]
    fn dispatch_preflight_emits_conflict_warning_for_overlapping_claims() {
        let repo = init_dispatch_repo();
        let runtime = runtime_for_dispatch_repo(repo.path());
        let (app, execution) = app_and_execution_with_runtime(runtime);
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

        execution
            .dispatch_preflight(&first_session.id, &first_run_id)
            .expect("first preflight should claim file");
        execution
            .dispatch_preflight(&second_session.id, &second_run_id)
            .expect("second preflight should warn");

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

    fn init_dispatch_repo() -> tempfile::TempDir {
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

    fn runtime_for_dispatch_repo(repo: &std::path::Path) -> crate::RuntimeService {
        crate::RuntimeService::from_host_platform_with_paths(
            ta_host_platform::detect_current_platform(),
            crate::RuntimeExecutionPaths {
                working_directory: repo.to_path_buf(),
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
            .selected_runtime_profile()
            .expect("profile");
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
                        parent_run_id: RunId::new("run-parent-dispatch").expect("parent id"),
                        parent_turn_id: ta_protocol::wire::AgentStreamTurnId::new("turn-dispatch")
                            .expect("turn id"),
                        output_contract: None,
                        model_id: None,
                        sandbox_profile: None,
                        recipe_id: None,
                        workspace_scope: crate::WorkspaceMode::WorktreeWrite,
                        cleanup_policy,
                        planned_write_files: planned_write_files
                            .into_iter()
                            .map(str::to_string)
                            .collect(),
                    },
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
                    detail: "seed dispatch child".to_string(),
                    output_contract: None,
                    recipe_id: None,
                    result: None,
                })],
                occurred_at_ms: current_time_ms(),
            })
            .expect("run should seed");
        execution
            .runtime
            .claim_live_run(run_id.clone(), session_id.clone());
    }
}
