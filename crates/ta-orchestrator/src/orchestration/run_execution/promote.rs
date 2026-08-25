use ta_policy::{Operation, evaluate_execution_context};
use ta_store::CommitRunTransition;

use super::*;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(crate) fn rehydrate_scheduler_on_boot(&self) -> Result<(), RunExecutionError> {
        let plan = {
            let store = self.store.lock().expect("app store should not be poisoned");
            self.runtime.rehydrate_scheduler_from_store(&*store)?
        };
        for (session_id, run_id) in plan.demote_to_queued {
            self.demote_waiting_run_to_queued(session_id, &run_id)?;
        }
        for (session_id, run_id) in plan.promote_from_queue {
            self.promote_queued_run(session_id, &run_id)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn seed_running_run_for_tests(
        &self,
        session_id: crate::SessionId,
        objective: String,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let run_id = RunId::new(format!("run-{}", uuid::Uuid::new_v4().simple()))
            .expect("generated run id should be valid");
        let disposition = self
            .runtime
            .schedule_run_start(&session_id, run_id.clone())
            .map_err(|error| match error {
                crate::RunSchedulerError::QueueFull(session_id) => {
                    RunExecutionError::RunQueueFull(session_id)
                }
            })?;
        assert!(
            matches!(disposition, crate::RunScheduleDisposition::StartNow),
            "seeded running run requires an empty scheduler slot"
        );
        let runtime_profile = self
            .runtime
            .selected_runtime_profile()
            .map_err(map_agent_runtime_error)?;
        let prepared_context = self.prepare_execution_context(
            &session_id,
            &run_id,
            &runtime_profile,
            ExecutionContextRequest::workspace_write(),
        )?;
        let (run, events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let committed = store.commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: RunProjection {
                    id: run_id.clone(),
                    session_id: session_id.clone(),
                    runtime_profile_id: runtime_profile.id.clone(),
                    objective,
                    status: RunStatus::Running,
                    harness: RunHarnessKind::Native,
                    source: RunSource::default(),
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
                events: vec![DaemonEvent::Run(crate::RunEvent {
                    run_id: run_id.clone(),
                    status: RunStatus::Running,
                    detail: "Seeded live run for owner-layer proof".to_string(),
                    output_contract: None,
                    recipe_id: None,
                    result: None,
                })],
                occurred_at_ms: current_time_ms(),
            })?;
            (committed.run, committed.events)
        };
        self.runtime.claim_live_run(run_id, session_id);
        Ok(RunMutationResult {
            run: project_run_summary(run),
            events,
        })
    }

    fn demote_waiting_run_to_queued(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
    ) -> Result<(), RunExecutionError> {
        let mut store = self.store.lock().expect("app store should not be poisoned");
        let Some(existing_run) = store.run(run_id)? else {
            return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
        };
        if existing_run.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                existing_run.id.as_str().to_string(),
            ));
        }
        if existing_run.status != RunStatus::WaitingForApproval {
            return Err(RunExecutionError::RunNotWaitingForApproval(
                existing_run.id.as_str().to_string(),
            ));
        }
        let recipe_id = recipe_id_for_run(&existing_run);
        store.commit_run_transition(CommitRunTransition {
            session_id,
            run: RunProjection {
                status: RunStatus::Queued,
                ..existing_run
            },
            events: vec![DaemonEvent::Run(crate::RunEvent {
                run_id: run_id.clone(),
                status: RunStatus::Queued,
                detail: "Queued after daemon restart reconciliation".to_string(),
                output_contract: None,
                recipe_id,
                result: None,
            })],
            occurred_at_ms: current_time_ms(),
        })?;
        Ok(())
    }

    pub(super) fn advance_ready_queue(
        &self,
        session_id: &crate::SessionId,
        completed_run_id: &RunId,
        terminal_status: RunStatus,
    ) -> Result<Vec<EventRecord>, RunExecutionError> {
        let mut promoted_events = Vec::new();
        let mut next_run_id =
            self.runtime
                .finish_scheduled_run(session_id, completed_run_id, terminal_status);
        while let Some(run_id) = next_run_id {
            let promoted = self.promote_queued_run(session_id.clone(), &run_id)?;
            let terminal = matches!(promoted.run.status, RunStatus::Failed);
            promoted_events.extend(promoted.events);
            next_run_id = if terminal {
                self.runtime
                    .finish_scheduled_run(session_id, &run_id, RunStatus::Failed)
            } else {
                None
            };
        }
        Ok(promoted_events)
    }

    fn promote_queued_run(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let operation = Operation::new(ApprovalScope::ProcessExec, "execute run");
        let queued_run = self.load_run_projection(run_id)?;
        if queued_run.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                queued_run.id.as_str().to_string(),
            ));
        }
        if queued_run.status != RunStatus::Queued {
            return Err(RunExecutionError::RunNotQueued(
                queued_run.id.as_str().to_string(),
            ));
        }
        let runtime_profile = self
            .runtime
            .runtime_profile(&queued_run.runtime_profile_id)
            .map_err(map_agent_runtime_error)?;
        let decision = evaluate_execution_context(&queued_run.execution_context, &operation);
        let (mut run, mut events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let Some(existing_run) = store.run(run_id)? else {
                return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
            };
            if existing_run.session_id != session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    existing_run.id.as_str().to_string(),
                ));
            }
            if existing_run.status != RunStatus::Queued {
                return Err(RunExecutionError::RunNotQueued(
                    existing_run.id.as_str().to_string(),
                ));
            }

            let recipe_id = recipe_id_for_run(&existing_run);
            let (status, events) =
                build_start_transition(existing_run.id.clone(), decision, recipe_id);
            let committed = store.commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: RunProjection {
                    status,
                    ..existing_run
                },
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
        Ok(RunMutationResult {
            run: project_run_summary(run),
            events,
        })
    }
}
