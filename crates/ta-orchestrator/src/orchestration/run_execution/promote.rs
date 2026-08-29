use ta_policy::{Operation, evaluate_execution_context};
use ta_store::CommitRunTransition;

use super::provider_sink::RunCompletionProjection;
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
        validated_selection: crate::orchestration::ValidatedRunSelection,
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
        let runtime_profile = validated_selection.runtime_profile();
        let route = validated_selection.route();
        let harness = validated_selection.execution_harness();
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
                    harness: run_harness_kind(&harness),
                    source: RunSource::User {
                        route: route.clone(),
                        output_contract: None,
                        model_id: route.model_id.clone(),
                        recipe_id: None,
                        attachments: Vec::new(),
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
                events: vec![DaemonEvent::Run(
                    crate::RunEvent::active(run_id.clone(), RunStatus::Running, None, None, None)
                        .expect("active status"),
                )],
                occurred_at_ms: current_time_ms(),
                auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
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
            user_turn: ta_store::UserTurnCommit::NoUserTurn,
            events: vec![DaemonEvent::Run(
                crate::RunEvent::active(run_id.clone(), RunStatus::Queued, None, recipe_id, None)
                    .expect("active status"),
            )],
            occurred_at_ms: current_time_ms(),
            auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
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

    pub(crate) fn promote_queued_run(
        &self,
        session_id: crate::SessionId,
        run_id: &RunId,
    ) -> Result<RunMutationResult, RunExecutionError> {
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
        let decision = match runtime_profile.execution_kind {
            ta_protocol::wire::RuntimeProfileExecutionKind::AgentRun => evaluate_execution_context(
                &queued_run.execution_context,
                &Operation::new(ApprovalScope::ProcessExec, "execute run"),
            ),
            ta_protocol::wire::RuntimeProfileExecutionKind::RealtimeVoice => {
                ta_policy::PolicyDecision::Allow
            }
        };
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
                user_turn: ta_store::UserTurnCommit::NoUserTurn,
                events,
                occurred_at_ms: current_time_ms(),
                auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
            })?;
            (committed.run, committed.events)
        };
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
                let failed = self.commit_failed_live_run_for_generation(
                    session_id.clone(),
                    &latest_run.id,
                    error.to_string(),
                    RunCompletionProjection::default(),
                    generation,
                )?;
                run = self.load_run_projection(&latest_run.id)?;
                events.extend(failed.events);
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
