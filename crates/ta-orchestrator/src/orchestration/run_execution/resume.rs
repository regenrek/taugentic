use ta_protocol::wire::{ResumeRunRequest, ResumeRunResult, ResumeRunState};
use taugentic_agent::AgentExecutionHarness;

use super::*;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub fn resume_run(
        &self,
        session_id: crate::SessionId,
        request: ResumeRunRequest,
    ) -> Result<ResumeRunResult, RunExecutionError> {
        let run = self.load_run_projection(&request.run_id)?;
        if run.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                run.id.as_str().to_string(),
            ));
        }
        let runtime_profile = self
            .runtime
            .runtime_profile(&run.runtime_profile_id)
            .map_err(map_agent_runtime_error)?;
        let current_harness = self
            .runtime
            .execution_harness_for_runtime_profile(&runtime_profile)
            .map_err(map_agent_runtime_error)?;
        if run.harness != RunHarnessKind::Native
            || !matches!(current_harness, AgentExecutionHarness::NativeLoop)
        {
            return Err(RunExecutionError::RunNotNativeHarness(
                run.id.as_str().to_string(),
            ));
        }

        let state = match run.status {
            RunStatus::Queued => ResumeRunState::Queued,
            RunStatus::Running | RunStatus::WaitingForApproval => {
                if !self.runtime.is_live_run_running(&run.id, &session_id) {
                    return Err(RunExecutionError::RunNotResumable(format!(
                        "{} terminated unexpectedly and has no live native owner",
                        run.id.as_str()
                    )));
                }
                ResumeRunState::Live
            }
            RunStatus::Completed
            | RunStatus::Failed
            | RunStatus::BudgetExceeded
            | RunStatus::Cancelled => {
                return Err(RunExecutionError::RunNotResumable(format!(
                    "{} is terminal ({:?})",
                    run.id.as_str(),
                    run.status
                )));
            }
        };

        Ok(ResumeRunResult {
            latest_event_seq: run.last_event_seq,
            run: project_run_record(run),
            state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::run_execution::test_support::*;
    use ta_protocol::wire::{
        ResumeRunRequest, ResumeRunState, RunHarnessKind, RunSource, RunStatus,
    };
    use ta_store::{CommitRepository, CommitRunTransition, ProjectionRepository};

    #[test]
    fn resume_run_returns_live_native_state() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Resume native run");
        let run = ensure_running_run(&app, &execution, &session.id, "Resume me");

        let resumed = execution
            .resume_run(
                session.id.clone(),
                ResumeRunRequest {
                    run_id: run.id.clone(),
                },
            )
            .expect("running native run should resume");

        assert_eq!(resumed.state, ResumeRunState::Live);
        assert_eq!(resumed.run.id, run.id);
        assert_eq!(resumed.run.harness, RunHarnessKind::Native);
        assert_eq!(resumed.latest_event_seq, resumed.run.last_event_seq);
        assert!(resumed.latest_event_seq.is_some());
    }

    #[test]
    fn resume_run_rejects_terminal_native_run() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Terminal resume");
        let run = ensure_running_run(&app, &execution, &session.id, "Finish me");
        {
            let mut store = execution
                .store
                .lock()
                .expect("app store should not be poisoned");
            let existing = store
                .run(&run.id)
                .expect("run lookup")
                .expect("run should exist");
            store
                .commit_run_transition(CommitRunTransition {
                    session_id: session.id.clone(),
                    run: RunProjection {
                        status: RunStatus::Completed,
                        ..existing
                    },
                    user_turn: ta_store::UserTurnCommit::NoUserTurn,
                    events: vec![DaemonEvent::Run(
                        crate::RunEvent::terminal(
                            run.id.clone(),
                            RunStatus::Completed,
                            crate::RunStatusReason::new("done").expect("reason"),
                            None,
                            None,
                            None,
                        )
                        .expect("completed is terminal"),
                    )],
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .expect("terminal run should persist");
        }

        let error = execution
            .resume_run(session.id.clone(), ResumeRunRequest { run_id: run.id })
            .expect_err("terminal run should not resume");

        assert!(matches!(error, RunExecutionError::RunNotResumable(_)));
    }

    #[test]
    fn resume_run_rejects_external_harness_run() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "External resume");
        let run = ensure_running_run_with_profile(
            &app,
            &execution,
            &session.id,
            "External run",
            "runtime-codex-safe",
        );

        let error = execution
            .resume_run(session.id.clone(), ResumeRunRequest { run_id: run.id })
            .expect_err("external run should not resume");

        assert!(matches!(error, RunExecutionError::RunNotNativeHarness(_)));
    }

    #[test]
    fn resume_run_preserves_native_subagent_parent_linkage() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Child resume");
        let parent = ensure_running_run(&app, &execution, &session.id, "Parent run");
        let child_run_id = RunId::new("run-child-resume").expect("child run id");
        {
            let mut store = execution
                .store
                .lock()
                .expect("app store should not be poisoned");
            store
                .commit_run_transition(CommitRunTransition {
                    session_id: session.id.clone(),
                    run: RunProjection {
                        id: child_run_id.clone(),
                        session_id: session.id.clone(),
                        runtime_profile_id: parent.runtime_profile_id.clone(),
                        objective: "Child run".to_string(),
                        status: RunStatus::Running,
                        harness: RunHarnessKind::Native,
                        source: RunSource::NativeSubagent {
                            route: ta_store::default_test_run_source().route().clone(),
                            parent_run_id: parent.id.clone(),
                            parent_turn_id: ta_protocol::wire::AgentStreamTurnId::new(
                                "turn-child-resume",
                            )
                            .expect("turn id"),
                            output_contract: None,
                            model_id: None,
                            recipe_id: None,
                            workspace_scope: crate::WorkspaceMode::WorkspaceWrite,
                            cleanup_policy: crate::WorktreeCleanupPolicy::DeleteOnSuccess,
                            planned_write_files: Vec::new(),
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
                    },
                    user_turn: ta_store::UserTurnCommit::NoUserTurn,
                    events: vec![DaemonEvent::Run(
                        crate::RunEvent::active(
                            child_run_id.clone(),
                            RunStatus::Running,
                            None,
                            None,
                            None,
                        )
                        .expect("active status"),
                    )],
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .expect("child run should persist");
        }
        execution
            .runtime
            .claim_live_run(child_run_id.clone(), session.id.clone());

        let resumed = execution
            .resume_run(
                session.id,
                ResumeRunRequest {
                    run_id: child_run_id,
                },
            )
            .expect("native child should resume");

        assert_eq!(resumed.state, ResumeRunState::Live);
        assert_eq!(resumed.run.parent_run_id, Some(parent.id));
    }
}
