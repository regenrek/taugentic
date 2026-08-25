use ta_policy::{Operation, evaluate_execution_context};
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
        let runtime_profile = self
            .runtime
            .selected_runtime_profile()
            .map_err(map_agent_runtime_error)?;
        let disposition = self
            .runtime
            .schedule_run_start(&session_id, run_id.clone())
            .map_err(|error| match error {
                crate::RunSchedulerError::QueueFull(session_id) => {
                    RunExecutionError::RunQueueFull(session_id)
                }
            })?;
        let fail_scheduled_run = |error| {
            self.runtime
                .finish_scheduled_run(&session_id, &run_id, RunStatus::Failed);
            error
        };
        let prepared_context = self
            .prepare_execution_context(
                &session_id,
                &run_id,
                &runtime_profile,
                ExecutionContextRequest::workspace_write(),
            )
            .map_err(fail_scheduled_run)?;
        let decision = evaluate_execution_context(
            &prepared_context.execution_context,
            &Operation::new(ApprovalScope::ProcessExec, "execute run"),
        );
        let harness = self
            .runtime
            .execution_harness_for_runtime_profile(&runtime_profile)
            .map_err(map_agent_runtime_error)
            .map_err(fail_scheduled_run)?;

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
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: runtime_profile.id.clone(),
                objective: objective.to_string(),
                status,
                harness: run_harness_kind(&harness),
                source: RunSource::User {
                    output_contract: resolved_command.output_contract,
                    model_id: resolved_command.model_id.clone(),
                    recipe_id: resolved_command.recipe_id.clone(),
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
            };
            let committed = store
                .commit_run_transition(CommitRunTransition {
                    session_id: session_id.clone(),
                    run: run.clone(),
                    events,
                    occurred_at_ms: current_time_ms(),
                })
                .map_err(|error| fail_scheduled_run(error.into()))?;
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
        let fork_initial_state = self.fork_initial_state_for_run(session_id, run_id)?;
        let run = self.load_run_projection(run_id)?;
        let output_contract = output_contract_for_run(&run);
        self.runtime
            .start_provider_run(
                crate::ProviderRunStart {
                    runtime_profile,
                    session_id,
                    run_id,
                    objective,
                    execution_context: Arc::new(run.execution_context),
                    fork_initial_state,
                    output_contract,
                    model_id: overrides.model_id.as_ref(),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::run_execution::test_support::*;
    use crate::{SessionId, StartRunCommand};
    use ta_protocol::wire::{OutputContractKind, RunSource, RunStatus};

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
                    workspace_id: ta_store::default_test_workspace_id(),
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
                    workspace_id: ta_store::default_test_workspace_id(),
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
    fn native_and_acp_runs_persist_the_same_workspace_context_identity() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Cross-harness context");

        select_runtime_profile(&app, "runtime-openai-safe");
        let native = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "Native context proof".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("native run should persist before approval");

        select_runtime_profile(&app, "runtime-codex-acp-safe");
        let acp = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "ACP context proof".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("ACP run should queue and persist");

        let native = execution
            .load_run_projection(&native.run.id)
            .expect("native run projection");
        let acp = execution
            .load_run_projection(&acp.run.id)
            .expect("ACP run projection");

        assert_eq!(native.harness, RunHarnessKind::Native);
        assert_eq!(acp.harness, RunHarnessKind::Acp);
        assert_eq!(
            acp.execution_context.workspace_id,
            native.execution_context.workspace_id
        );
        assert_eq!(
            acp.execution_context.workspace_root,
            native.execution_context.workspace_root
        );
        assert_eq!(
            acp.execution_context.effective_cwd,
            native.execution_context.effective_cwd
        );
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
                    workspace_id: ta_store::default_test_workspace_id(),
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
                    workspace_id: ta_store::default_test_workspace_id(),
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
        assert!(matches!(
            &run.source,
            RunSource::User {
                recipe_id: Some(recipe_id),
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
                    workspace_id: ta_store::default_test_workspace_id(),
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
                    workspace_id: ta_store::default_test_workspace_id(),
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
                    workspace_id: ta_store::default_test_workspace_id(),
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
}
