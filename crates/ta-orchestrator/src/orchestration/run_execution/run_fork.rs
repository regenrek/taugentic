use ta_policy::{Operation, evaluate_execution_context};
use ta_protocol::wire::{
    ApprovalScope, ForkRunRequest, ForkRunResult, RunHarnessKind, RunSource, RunStatus,
};
use ta_store::{
    CommitRunTransition, PersistenceStore, RunEventRangeQuery, RunProjection, UserTurnCommit,
};
use taugentic_agent::AgentExecutionHarness;
use uuid::Uuid;

use super::*;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub fn fork_run(
        &self,
        session_id: crate::SessionId,
        request: ForkRunRequest,
    ) -> Result<ForkRunResult, RunExecutionError> {
        if request.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                request.parent_run_id.as_str().to_string(),
            ));
        }

        let parent = {
            let store = self.store.lock().expect("app store should not be poisoned");
            let Some(parent) = store.run(&request.parent_run_id)? else {
                return Err(RunExecutionError::RunNotFound(
                    request.parent_run_id.as_str().to_string(),
                ));
            };
            if parent.session_id != session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    parent.id.as_str().to_string(),
                ));
            }
            if parent.harness != RunHarnessKind::Native {
                return Err(RunExecutionError::RunNotNativeHarness(
                    parent.id.as_str().to_string(),
                ));
            }
            let latest_event_seq = parent.last_event_seq.ok_or_else(|| {
                RunExecutionError::RunForkPointNotFound(format!(
                    "{}:{}",
                    parent.id.as_str(),
                    request.parent_event_seq
                ))
            })?;
            if request.parent_event_seq == 0 || request.parent_event_seq > latest_event_seq {
                return Err(RunExecutionError::RunForkPointNotFound(format!(
                    "{}:{}",
                    parent.id.as_str(),
                    request.parent_event_seq
                )));
            }
            let fork_event = store.read_run_events(&RunEventRangeQuery {
                session_id: session_id.clone(),
                run_id: parent.id.clone(),
                after_sequence: request.parent_event_seq.checked_sub(1),
                limit: 1,
            })?;
            if fork_event
                .records
                .first()
                .is_none_or(|record| record.sequence != request.parent_event_seq)
            {
                return Err(RunExecutionError::RunForkPointNotFound(format!(
                    "{}:{}",
                    parent.id.as_str(),
                    request.parent_event_seq
                )));
            }
            super::fork_snapshot::native_history_initial_state_for_parent(
                &*store,
                &session_id,
                &parent,
                request.parent_event_seq,
            )?;
            parent
        };

        let runtime_profile = self
            .runtime
            .runtime_profile(&parent.runtime_profile_id)
            .map_err(map_agent_runtime_error)?;
        if parent.harness != RunHarnessKind::Native {
            return Err(RunExecutionError::RunNotNativeHarness(
                parent.id.as_str().to_string(),
            ));
        }

        let objective = match request.objective.as_deref().map(str::trim) {
            Some("") => return Err(RunExecutionError::EmptyRunObjective),
            Some(objective) => objective.to_string(),
            None => parent.objective.clone(),
        };
        if objective.trim().is_empty() {
            return Err(RunExecutionError::EmptyRunObjective);
        }
        let user_turn = UserTurnCommit::Append {
            text: objective.clone(),
            attachments: Vec::new(),
        };

        let fork_run_id = crate::RunId::new(format!("run-{}", Uuid::new_v4().simple()))
            .expect("generated run id should be valid");
        let disposition = self
            .runtime
            .schedule_run_start(&session_id, fork_run_id.clone())
            .map_err(|error| match error {
                crate::RunSchedulerError::QueueFull(session_id) => {
                    RunExecutionError::RunQueueFull(session_id)
                }
            })?;
        let fail_scheduled_run = |error| {
            self.runtime
                .finish_scheduled_run(&session_id, &fork_run_id, RunStatus::Failed);
            error
        };
        let workspace_mode =
            workspace_mode_for_fork(&parent.execution_context).map_err(fail_scheduled_run)?;
        let prepared_context = self
            .prepare_child_execution_context(
                &session_id,
                &fork_run_id,
                &parent.execution_context,
                ExecutionContextRequest {
                    workspace_mode,
                    cleanup_policy: ta_protocol::wire::WorktreeCleanupPolicy::DeleteOnSuccess,
                    planned_write_files: Vec::new(),
                },
            )
            .map_err(fail_scheduled_run)?;
        let execution_harness = self
            .runtime
            .execution_harness_for_runtime_profile(&runtime_profile)
            .map_err(map_agent_runtime_error)
            .map_err(fail_scheduled_run)?;
        if !matches!(execution_harness, AgentExecutionHarness::NativeLoop) {
            return Err(fail_scheduled_run(RunExecutionError::RunNotNativeHarness(
                parent.id.as_str().to_string(),
            )));
        }
        let decision = evaluate_execution_context(
            &prepared_context.execution_context,
            &Operation::new(ApprovalScope::ProcessExec, "execute forked native run"),
        );

        let (mut run, events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let (status, events) = match disposition {
                crate::RunScheduleDisposition::StartNow => {
                    build_start_transition(fork_run_id.clone(), decision, None)
                }
                crate::RunScheduleDisposition::Queued { position } => {
                    build_queue_transition(fork_run_id.clone(), position, None)
                }
            };
            let fork = RunProjection {
                id: fork_run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: runtime_profile.id.clone(),
                objective,
                status,
                harness: RunHarnessKind::Native,
                source: RunSource::Forked {
                    route: parent.source.route().clone(),
                    parent_run_id: parent.id.clone(),
                    parent_event_seq: request.parent_event_seq,
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
                    run: fork,
                    user_turn,
                    events,
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .map_err(|error| fail_scheduled_run(error.into()))?;
            (committed.run, committed.events)
        };
        self.publish_records(&events);

        if run.status == RunStatus::Running {
            let generation = self
                .runtime
                .claim_live_run(run.id.clone(), session_id.clone());
            let start_result = self.start_provider_execution(
                &session_id,
                &run.id,
                &runtime_profile,
                run.source.route(),
                generation,
            );
            let latest_run = self.load_run_projection(&run.id)?;
            if let Err(error) = start_result
                && latest_run.status == RunStatus::Running
            {
                self.fail_live_run_and_publish_for_generation(
                    session_id.clone(),
                    &latest_run.id,
                    error.to_string(),
                    generation,
                )?;
                run = self.load_run_projection(&latest_run.id)?;
            } else if latest_run.status != RunStatus::Cancelled {
                run = latest_run;
            }
        }

        Ok(ForkRunResult {
            run: project_run_record(run),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{SessionId, orchestration::run_execution::test_support::*};
    use ta_protocol::wire::{
        DaemonEvent, ForkRunRequest, RunHarnessKind, RunId, RunSource, RunStatus,
    };
    use ta_store::{CommitRepository, InMemoryStore, ProjectionRepository};

    use super::*;

    fn fork_request(
        session_id: &SessionId,
        parent_run_id: &RunId,
        parent_event_seq: u64,
    ) -> ForkRunRequest {
        ForkRunRequest {
            session_id: session_id.clone(),
            parent_run_id: parent_run_id.clone(),
            parent_event_seq,
            objective: Some("Forked objective".to_string()),
        }
    }

    fn last_event_seq(execution: &RunExecutionService<InMemoryStore>, run_id: &RunId) -> u64 {
        execution
            .store
            .lock()
            .expect("store should not poison")
            .run(run_id)
            .expect("run lookup should work")
            .expect("run should exist")
            .last_event_seq
            .expect("run should have durable event")
    }

    fn stored_run_status(
        execution: &RunExecutionService<InMemoryStore>,
        run_id: &RunId,
    ) -> RunStatus {
        stored_run(execution, run_id).status
    }

    fn stored_run(execution: &RunExecutionService<InMemoryStore>, run_id: &RunId) -> RunProjection {
        execution
            .store
            .lock()
            .expect("store should not poison")
            .run(run_id)
            .expect("run lookup should work")
            .expect("run should exist")
    }

    fn set_parent_status(
        execution: &RunExecutionService<InMemoryStore>,
        session_id: &SessionId,
        parent_run_id: &RunId,
        status: RunStatus,
    ) {
        let mut store = execution.store.lock().expect("store should not poison");
        let existing = store
            .run(parent_run_id)
            .expect("run lookup should work")
            .expect("parent should exist");
        let event = match status {
            RunStatus::Queued | RunStatus::Running | RunStatus::WaitingForApproval => {
                crate::RunEvent::active(parent_run_id.clone(), status, None, None, None)
                    .expect("parent status should be active")
            }
            RunStatus::Completed
            | RunStatus::Failed
            | RunStatus::BudgetExceeded
            | RunStatus::Cancelled => crate::RunEvent::terminal(
                parent_run_id.clone(),
                status,
                crate::RunStatusReason::new(format!("parent {status:?}"))
                    .expect("parent terminal status reason should be valid"),
                None,
                None,
                None,
            )
            .expect("parent status should be terminal"),
        };
        store
            .commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: RunProjection { status, ..existing },
                user_turn: ta_store::UserTurnCommit::NoUserTurn,
                events: vec![DaemonEvent::Run(event)],
                occurred_at_ms: current_time_ms(),
                auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
            })
            .expect("status update should persist");
    }

    #[test]
    fn fork_run_records_lineage_for_active_native_parent() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let selection = validated_runtime_selection(&app, "runtime-openai-safe");
        let session = open_session(&app, "Active fork");
        let parent = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "Parent objective".to_string(),
                selection,
            )
            .expect("parent should seed");
        let parent_route = stored_run(&execution, &parent.run.id)
            .source
            .route()
            .clone();
        let parent_event_seq = last_event_seq(&execution, &parent.run.id);

        let fork = execution
            .fork_run(
                session.id.clone(),
                fork_request(&session.id, &parent.run.id, parent_event_seq),
            )
            .expect("active native run should fork");

        assert_eq!(fork.run.harness, RunHarnessKind::Native);
        assert_eq!(fork.run.status, RunStatus::Queued);
        assert_eq!(fork.run.parent_run_id, Some(parent.run.id.clone()));
        assert_eq!(
            fork.run.source,
            RunSource::Forked {
                route: parent_route,
                parent_run_id: parent.run.id.clone(),
                parent_event_seq,
            }
        );
        let stored = execution
            .store
            .lock()
            .expect("store should not poison")
            .run(&fork.run.id)
            .expect("fork lookup should work")
            .expect("fork should persist");
        assert_eq!(stored.source, fork.run.source);
    }

    #[test]
    fn cancel_parent_does_not_cancel_forked_run() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let selection = validated_runtime_selection(&app, "runtime-openai-safe");
        let session = open_session(&app, "Forked run parent cancel");
        let parent = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "Parent objective".to_string(),
                selection,
            )
            .expect("parent should seed");
        attach_noop_handle(&execution, &parent.run.id);
        let parent_event_seq = last_event_seq(&execution, &parent.run.id);
        let fork = execution
            .fork_run(
                session.id.clone(),
                fork_request(&session.id, &parent.run.id, parent_event_seq),
            )
            .expect("fork should queue behind running parent");

        let cancelled = execution
            .cancel_run(
                session.id.clone(),
                approval_actor(),
                &parent.run.id,
                Some("cancel parent".to_string()),
            )
            .expect("parent cancel should not cascade to forked run");
        let fork_status = stored_run_status(&execution, &fork.run.id);

        assert!(matches!(fork.run.source, RunSource::Forked { .. }));
        assert_eq!(cancelled.run.status, RunStatus::Cancelled);
        assert_ne!(fork_status, RunStatus::Cancelled);
        assert!(cancelled.events.iter().all(|record| {
            !matches!(
                &record.payload,
                DaemonEvent::Run(crate::RunEvent::Status(event))
                    if event.run_id() == &fork.run.id && event.status() == RunStatus::Cancelled
            )
        }));
    }

    #[test]
    fn cancel_forked_run_does_not_cancel_parent() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let selection = validated_runtime_selection(&app, "runtime-openai-safe");
        let session = open_session(&app, "Forked run cancel");
        let parent = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "Parent objective".to_string(),
                selection,
            )
            .expect("parent should seed");
        let parent_event_seq = last_event_seq(&execution, &parent.run.id);
        let fork = execution
            .fork_run(
                session.id.clone(),
                fork_request(&session.id, &parent.run.id, parent_event_seq),
            )
            .expect("fork should queue behind running parent");

        let cancelled = execution
            .cancel_run(
                session.id.clone(),
                approval_actor(),
                &fork.run.id,
                Some("cancel fork".to_string()),
            )
            .expect("fork cancel should not affect parent");
        let parent_status = stored_run_status(&execution, &parent.run.id);

        assert!(matches!(fork.run.source, RunSource::Forked { .. }));
        assert_eq!(cancelled.run.status, RunStatus::Cancelled);
        assert_eq!(parent_status, RunStatus::Running);
    }

    #[test]
    fn cancel_running_forked_run_does_not_cancel_parent() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let selection = validated_runtime_selection(&app, "runtime-openai-safe");
        let session = open_session(&app, "Running fork cancel");
        let parent = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "Parent objective".to_string(),
                selection,
            )
            .expect("parent should seed");
        let parent_event_seq = last_event_seq(&execution, &parent.run.id);
        let fork = execution
            .fork_run(
                session.id.clone(),
                fork_request(&session.id, &parent.run.id, parent_event_seq),
            )
            .expect("fork should queue behind running parent");
        assert_eq!(fork.run.status, RunStatus::Queued);

        {
            let mut store = execution.store.lock().expect("store should not poison");
            let queued_fork = store
                .run(&fork.run.id)
                .expect("fork lookup should work")
                .expect("fork should persist");
            store
                .commit_run_transition(CommitRunTransition {
                    session_id: session.id.clone(),
                    run: RunProjection {
                        status: RunStatus::Running,
                        ..queued_fork
                    },
                    user_turn: ta_store::UserTurnCommit::NoUserTurn,
                    events: vec![DaemonEvent::Run(
                        crate::RunEvent::active(
                            fork.run.id.clone(),
                            RunStatus::Running,
                            None,
                            None,
                            None,
                        )
                        .expect("seeded running fork status should be active"),
                    )],
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .expect("fork should transition to running");
        }
        execution
            .runtime
            .claim_live_run(fork.run.id.clone(), session.id.clone());
        attach_noop_handle(&execution, &fork.run.id);
        let parent_before_cancel = stored_run(&execution, &parent.run.id);

        let cancelled = execution
            .cancel_run(
                session.id.clone(),
                approval_actor(),
                &fork.run.id,
                Some("cancel running fork".to_string()),
            )
            .expect("running fork cancel should not affect parent");
        let parent_after_cancel = stored_run(&execution, &parent.run.id);

        assert!(matches!(fork.run.source, RunSource::Forked { .. }));
        assert_eq!(cancelled.run.status, RunStatus::Cancelled);
        assert_eq!(parent_after_cancel, parent_before_cancel);
        assert_eq!(parent_after_cancel.status, RunStatus::Running);
        assert!(cancelled.events.iter().all(|record| {
            !matches!(
                &record.payload,
                DaemonEvent::Run(crate::RunEvent::Status(event))
                    if event.run_id() == &parent.run.id && event.status() == RunStatus::Cancelled
            )
        }));
    }

    #[test]
    fn fork_run_allows_completed_and_failed_native_parents() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let selection = validated_runtime_selection(&app, "runtime-openai-safe");
        let session = open_session(&app, "Terminal fork");
        let completed = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "Completed parent".to_string(),
                selection,
            )
            .expect("completed parent should seed");
        let completed_event_seq = last_event_seq(&execution, &completed.run.id);
        set_parent_status(
            &execution,
            &session.id,
            &completed.run.id,
            RunStatus::Completed,
        );

        let completed_fork = execution
            .fork_run(
                session.id.clone(),
                fork_request(&session.id, &completed.run.id, completed_event_seq),
            )
            .expect("completed parent should fork");
        let failed_run_id = RunId::new("run-failed-parent").expect("run id");
        {
            let mut store = execution.store.lock().expect("store should not poison");
            store
                .commit_run_transition(CommitRunTransition {
                    session_id: session.id.clone(),
                    run: RunProjection {
                        id: failed_run_id.clone(),
                        session_id: session.id.clone(),
                        runtime_profile_id: completed.run.runtime_profile_id.clone(),
                        objective: "Failed parent".to_string(),
                        status: RunStatus::Failed,
                        harness: RunHarnessKind::Native,
                        source: ta_store::default_test_run_source(),
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
                        crate::RunEvent::terminal(
                            failed_run_id.clone(),
                            RunStatus::Failed,
                            crate::RunStatusReason::new("failed parent")
                                .expect("failed parent reason should be valid"),
                            None,
                            None,
                            None,
                        )
                        .expect("failed parent status should be terminal"),
                    )],
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .expect("failed parent should persist");
        }
        let failed_event_seq = last_event_seq(&execution, &failed_run_id);

        let failed_fork = execution
            .fork_run(
                session.id.clone(),
                fork_request(&session.id, &failed_run_id, failed_event_seq),
            )
            .expect("failed parent should fork");

        assert!(matches!(
            completed_fork.run.source,
            RunSource::Forked { .. }
        ));
        assert!(matches!(failed_fork.run.source, RunSource::Forked { .. }));
    }

    #[test]
    fn fork_run_rejects_invalid_parent_event_seq() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let selection = validated_runtime_selection(&app, "runtime-openai-safe");
        let session = open_session(&app, "Invalid fork point");
        let parent = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "Parent objective".to_string(),
                selection,
            )
            .expect("parent should seed");
        let last_event_seq = last_event_seq(&execution, &parent.run.id);

        let error = execution
            .fork_run(
                session.id.clone(),
                fork_request(&session.id, &parent.run.id, last_event_seq + 1),
            )
            .expect_err("future fork point must fail closed");

        assert!(matches!(error, RunExecutionError::RunForkPointNotFound(_)));
    }

    #[test]
    fn fork_run_rejects_external_parent_harness() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let selection = validated_runtime_selection(&app, "runtime-codex-safe");
        let session = open_session(&app, "External fork");
        let parent = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "External parent".to_string(),
                selection,
            )
            .expect("parent should seed");
        let parent_event_seq = last_event_seq(&execution, &parent.run.id);

        let error = execution
            .fork_run(
                session.id.clone(),
                fork_request(&session.id, &parent.run.id, parent_event_seq),
            )
            .expect_err("external parent must fail closed");

        assert!(matches!(error, RunExecutionError::RunNotNativeHarness(_)));
    }

    #[test]
    fn fork_run_rejects_non_run_event_fork_point() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let selection = validated_runtime_selection(&app, "runtime-openai-safe");
        let session = open_session(&app, "Non-run fork point");
        let parent = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "Parent objective".to_string(),
                selection,
            )
            .expect("parent should seed");

        let error = execution
            .fork_run(
                session.id.clone(),
                fork_request(&session.id, &parent.run.id, 1),
            )
            .expect_err("session event must not be a valid run fork point");

        assert!(matches!(error, RunExecutionError::RunForkPointNotFound(_)));
    }
}
