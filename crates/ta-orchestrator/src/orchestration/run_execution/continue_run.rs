use ta_policy::{Operation, evaluate_execution_context};
use ta_protocol::wire::{
    ApprovalScope, ContinueRunRequest, ContinueRunResult, RunHarnessKind, RunSource, RunStatus,
};
use ta_store::{CommitRunTransition, PersistenceStore, UserTurnCommit};
use taugentic_agent::AgentExecutionHarness;

use super::*;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub fn continue_run(
        &self,
        session_id: crate::SessionId,
        request: ContinueRunRequest,
    ) -> Result<ContinueRunResult, RunExecutionError> {
        if request.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                request.run_id.as_str().to_string(),
            ));
        }
        let message = request.message.trim().to_string();
        if message.is_empty() {
            return Err(RunExecutionError::EmptyRunObjective);
        }
        let parent = self.load_run_projection(&request.run_id)?;
        if parent.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                parent.id.as_str().to_string(),
            ));
        }
        if parent.harness != RunHarnessKind::Native
            || !matches!(parent.source, RunSource::Forked { .. })
        {
            return Err(RunExecutionError::RunNotNativeHarness(
                parent.id.as_str().to_string(),
            ));
        }
        if !matches!(
            parent.status,
            RunStatus::Completed
                | RunStatus::Failed
                | RunStatus::BudgetExceeded
                | RunStatus::Cancelled
        ) {
            return Err(RunExecutionError::RunNotResumable(
                parent.id.as_str().to_string(),
            ));
        }
        let runtime_profile = self
            .runtime
            .runtime_profile(&parent.runtime_profile_id)
            .map_err(map_agent_runtime_error)?;
        if !matches!(
            self.runtime
                .execution_harness_for_runtime_profile(&runtime_profile)
                .map_err(map_agent_runtime_error)?,
            AgentExecutionHarness::NativeLoop
        ) {
            return Err(RunExecutionError::RunNotNativeHarness(
                parent.id.as_str().to_string(),
            ));
        }
        let disposition = self
            .runtime
            .schedule_run_start(&session_id, parent.id.clone())
            .map_err(|error| match error {
                crate::RunSchedulerError::QueueFull(session_id) => {
                    RunExecutionError::RunQueueFull(session_id)
                }
            })?;
        let fail_scheduled_run = |error: RunExecutionError| {
            self.runtime
                .finish_scheduled_run(&session_id, &parent.id, RunStatus::Failed);
            error
        };
        let decision = evaluate_execution_context(
            &parent.execution_context,
            &Operation::new(ApprovalScope::ProcessExec, "continue forked native run"),
        );
        let (status, events) = match disposition {
            crate::RunScheduleDisposition::StartNow => {
                build_start_transition(parent.id.clone(), decision, None)
            }
            crate::RunScheduleDisposition::Queued { position } => {
                build_queue_transition(parent.id.clone(), position, None)
            }
        };
        let mut next = parent.clone();
        // The immutable source/context/route remain untouched. The submitted
        // payload below is the sole input to durable AgentUserRow creation.
        next.objective = message.clone();
        next.status = status;
        let committed = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            store
                .commit_run_transition(CommitRunTransition {
                    session_id: session_id.clone(),
                    run: next,
                    user_turn: UserTurnCommit::Append {
                        text: message,
                        attachments: Vec::new(),
                    },
                    events,
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .map_err(|error| fail_scheduled_run(RunExecutionError::from(error)))?
        };
        self.publish_records(&committed.events);
        let mut run = committed.run;
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
            let latest = self.load_run_projection(&run.id)?;
            if let Err(error) = start_result
                && latest.status == RunStatus::Running
            {
                self.fail_live_run_and_publish_for_generation(
                    session_id.clone(),
                    &latest.id,
                    error.to_string(),
                    generation,
                )?;
                run = self.load_run_projection(&latest.id)?;
            } else if latest.status != RunStatus::Cancelled {
                run = latest;
            }
        }
        Ok(ContinueRunResult {
            run: project_run_record(run),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::AppService;
    use ta_protocol::wire::{
        AgentStreamEvent, AgentStreamFrame, AgentStreamTurnId, DaemonEvent, ForkRunRequest,
        StreamEmission,
    };
    use ta_store::{
        CommitRepository, EventLogRepository, ProjectionRepository, SessionAgentTurnsPageQuery,
    };

    fn assistant_events(run_id: &RunId, turn: &str, text: &str) -> Vec<DaemonEvent> {
        [
            AgentStreamFrame::AssistantTurnStarted,
            AgentStreamFrame::AssistantMessageDelta {
                delta: text.to_string(),
            },
            AgentStreamFrame::AssistantTurnCompleted,
        ]
        .into_iter()
        .map(|frame| {
            DaemonEvent::AgentStream(AgentStreamEvent {
                run_id: run_id.clone(),
                emission: StreamEmission {
                    turn_id: Some(AgentStreamTurnId::new(turn).expect("turn")),
                    item_id: None,
                    fragment_sequence: None,
                    frame,
                },
            })
        })
        .collect()
    }

    fn append(
        execution: &RunExecutionService<ta_store::InMemoryStore>,
        session_id: &crate::SessionId,
        run_id: &RunId,
        events: Vec<DaemonEvent>,
    ) -> Vec<ta_store::EventRecord> {
        let mut store = execution.store.lock().expect("store");
        let run = store.run(run_id).expect("read").expect("run");
        store
            .commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run,
                user_turn: ta_store::UserTurnCommit::NoUserTurn,
                events,
                occurred_at_ms: current_time_ms(),
                auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
            })
            .expect("append")
            .events
    }

    fn terminal(
        execution: &RunExecutionService<ta_store::InMemoryStore>,
        session_id: &crate::SessionId,
        run_id: &RunId,
    ) {
        let mut store = execution.store.lock().expect("store");
        let run = store.run(run_id).expect("read").expect("run");
        store
            .commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: RunProjection {
                    status: RunStatus::Completed,
                    ..run
                },
                user_turn: ta_store::UserTurnCommit::NoUserTurn,
                events: vec![DaemonEvent::Run(
                    crate::RunEvent::terminal(
                        run_id.clone(),
                        RunStatus::Completed,
                        crate::RunStatusReason::new("complete").expect("reason"),
                        None,
                        None,
                        None,
                    )
                    .expect("completed is terminal"),
                )],
                occurred_at_ms: current_time_ms(),
                auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
            })
            .expect("terminal");
    }

    #[test]
    fn interactive_child_continuation_rehydrates_full_branch_history() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Continuation history");
        let parent = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "parent prompt".to_string(),
                validated_runtime_selection(&app, "runtime-openai-safe"),
            )
            .expect("parent");
        let parent_events = append(
            &execution,
            &session.id,
            &parent.run.id,
            assistant_events(&parent.run.id, "parent-turn", "parent answer"),
        );
        let boundary = parent_events.last().expect("boundary").sequence;
        terminal(&execution, &session.id, &parent.run.id);
        let fork = execution
            .fork_run(
                session.id.clone(),
                ForkRunRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent.run.id.clone(),
                    parent_event_seq: boundary,
                    objective: Some("child prompt".to_string()),
                },
            )
            .expect("fork");
        let child_events = append(
            &execution,
            &session.id,
            &fork.run.id,
            assistant_events(&fork.run.id, "child-turn", "child answer"),
        );
        let child_boundary = child_events.last().expect("child boundary").sequence;
        terminal(&execution, &session.id, &fork.run.id);
        let grandchild = execution
            .fork_run(
                session.id.clone(),
                ForkRunRequest {
                    session_id: session.id.clone(),
                    parent_run_id: fork.run.id.clone(),
                    parent_event_seq: child_boundary,
                    objective: Some("grandchild prompt".to_string()),
                },
            )
            .expect("nested fork");
        append(
            &execution,
            &session.id,
            &grandchild.run.id,
            assistant_events(&grandchild.run.id, "grandchild-turn", "grandchild answer"),
        );
        terminal(&execution, &session.id, &grandchild.run.id);

        let initial = execution
            .continuation_initial_state_for_run(&session.id, &grandchild.run.id)
            .expect("history");
        let contents = initial
            .messages
            .into_iter()
            .map(|message| message.content)
            .collect::<Vec<_>>();
        assert_eq!(
            contents,
            vec![
                "parent answer",
                "child prompt",
                "child answer",
                "grandchild prompt",
                "grandchild answer"
            ]
        );

        let before = execution
            .load_run_projection(&grandchild.run.id)
            .expect("stored grandchild");
        let continued = execution
            .continue_run(
                session.id.clone(),
                ContinueRunRequest {
                    session_id: session.id.clone(),
                    run_id: grandchild.run.id.clone(),
                    message: "continue only this branch".to_string(),
                },
            )
            .expect("continuation");
        assert_eq!(continued.run.execution_context, before.execution_context);
        assert_eq!(continued.run.source.route(), before.source.route());

        let rows = execution
            .store
            .lock()
            .expect("store")
            .session_agent_turns_page(&SessionAgentTurnsPageQuery {
                session_id: session.id.clone(),
                before_sequence: None,
                limit: 100,
            })
            .expect("turn rows")
            .rows;
        let mut continuation_turns = rows
            .into_iter()
            .filter_map(|row| match row {
                ta_protocol::wire::AgentTurnRow::User(row) if row.run_id == grandchild.run.id => {
                    Some((row.cursor.sequence, row.text))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        continuation_turns.sort_by_key(|(sequence, _)| *sequence);
        let continuation_turns = continuation_turns
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>();
        assert_eq!(
            continuation_turns,
            vec!["grandchild prompt", "continue only this branch"]
        );
    }

    #[test]
    fn same_message_continuation_persists_distinct_durable_turn() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Repeated continuation");
        let parent = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "parent prompt".to_string(),
                validated_runtime_selection(&app, "runtime-openai-safe"),
            )
            .expect("parent");
        let parent_events = append(
            &execution,
            &session.id,
            &parent.run.id,
            assistant_events(&parent.run.id, "parent-turn", "parent answer"),
        );
        terminal(&execution, &session.id, &parent.run.id);
        let fork = execution
            .fork_run(
                session.id.clone(),
                ForkRunRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent.run.id,
                    parent_event_seq: parent_events.last().expect("boundary").sequence,
                    objective: Some("same continuation message".to_string()),
                },
            )
            .expect("fork");
        terminal(&execution, &session.id, &fork.run.id);
        execution
            .continue_run(
                session.id.clone(),
                ContinueRunRequest {
                    session_id: session.id.clone(),
                    run_id: fork.run.id.clone(),
                    message: "same continuation message".to_string(),
                },
            )
            .expect("continuation");

        let rows = execution
            .store
            .lock()
            .expect("store")
            .session_agent_turns_page(&SessionAgentTurnsPageQuery {
                session_id: session.id,
                before_sequence: None,
                limit: 100,
            })
            .expect("turn rows")
            .rows;
        let turns = rows
            .into_iter()
            .filter_map(|row| match row {
                ta_protocol::wire::AgentTurnRow::User(row) if row.run_id == fork.run.id => {
                    Some(row.text)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            turns
                .iter()
                .filter(|text| text.as_str() == "same continuation message")
                .count(),
            2
        );
    }

    fn queued_continuation_fixture() -> (
        AppService<ta_store::InMemoryStore>,
        RunExecutionService<ta_store::InMemoryStore>,
        crate::SessionId,
        RunId,
        RunId,
    ) {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Queued continuation history");
        let parent = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "parent prompt".to_string(),
                validated_runtime_selection(&app, "runtime-openai-safe"),
            )
            .expect("parent");
        let parent_events = append(
            &execution,
            &session.id,
            &parent.run.id,
            assistant_events(&parent.run.id, "parent-turn", "parent answer"),
        );
        let parent_boundary = parent_events.last().expect("parent boundary").sequence;
        terminal(&execution, &session.id, &parent.run.id);
        execution
            .runtime
            .finish_scheduled_run(&session.id, &parent.run.id, RunStatus::Completed);

        let fork = execution
            .fork_run(
                session.id.clone(),
                ForkRunRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent.run.id,
                    parent_event_seq: parent_boundary,
                    objective: Some("child prompt".to_string()),
                },
            )
            .expect("fork");
        append(
            &execution,
            &session.id,
            &fork.run.id,
            assistant_events(&fork.run.id, "child-turn", "child answer"),
        );
        terminal(&execution, &session.id, &fork.run.id);
        execution
            .runtime
            .finish_scheduled_run(&session.id, &fork.run.id, RunStatus::Completed);

        let blocker = execution
            .seed_running_run_for_tests(
                session.id.clone(),
                "queue blocker".to_string(),
                validated_runtime_selection(&app, "runtime-openai-safe"),
            )
            .expect("blocker");
        let queued = execution
            .continue_run(
                session.id.clone(),
                ContinueRunRequest {
                    session_id: session.id.clone(),
                    run_id: fork.run.id.clone(),
                    message: "queued continuation".to_string(),
                },
            )
            .expect("queued continuation");
        assert_eq!(queued.run.status, RunStatus::Queued);

        let initial = execution
            .native_history_initial_state_for_run(&session.id, &fork.run.id)
            .expect("canonical dispatch history")
            .expect("fork state");
        assert_eq!(
            initial
                .messages
                .into_iter()
                .map(|message| message.content)
                .collect::<Vec<_>>(),
            vec!["parent answer", "child prompt", "child answer"]
        );
        (app, execution, session.id, blocker.run.id, fork.run.id)
    }

    #[test]
    fn queued_terminal_fork_continuation_rehydrates_full_branch_history() {
        let (app, execution, session_id, blocker_run_id, continuation_run_id) =
            queued_continuation_fixture();
        app.patch_agent_runtime_profile(&crate::DaemonAgentRuntimePatchProfileParams {
            runtime_profile_id: crate::RuntimeProfileId::new("runtime-openai-safe")
                .expect("runtime profile"),
            patch: crate::RuntimeProfilePatch {
                policy_mode: Some(crate::RuntimePolicyMode::Allow),
                ..Default::default()
            },
        })
        .expect("allow direct native dispatch");

        terminal(&execution, &session_id, &blocker_run_id);
        execution
            .advance_ready_queue(&session_id, &blocker_run_id, RunStatus::Completed)
            .expect("queue promotion");

        let promoted = execution
            .load_run_projection(&continuation_run_id)
            .expect("promoted continuation");
        assert_ne!(promoted.status, RunStatus::Queued);
    }

    #[test]
    fn restart_rehydration_promotes_queued_terminal_fork_with_canonical_history() {
        let (_app, execution, session_id, blocker_run_id, continuation_run_id) =
            queued_continuation_fixture();
        terminal(&execution, &session_id, &blocker_run_id);

        let restarted_runtime = crate::RuntimeService::bootstrap();
        let restarted = AppService::from_runtime(execution.store.clone(), &restarted_runtime);
        restarted
            .patch_agent_runtime_profile(&crate::DaemonAgentRuntimePatchProfileParams {
                runtime_profile_id: crate::RuntimeProfileId::new("runtime-openai-safe")
                    .expect("runtime profile"),
                patch: crate::RuntimeProfilePatch {
                    policy_mode: Some(crate::RuntimePolicyMode::Allow),
                    ..Default::default()
                },
            })
            .expect("allow rehydrated native dispatch");
        restarted
            .rehydrate_run_scheduler_on_boot()
            .expect("scheduler rehydration");

        let promoted = restarted
            .run_execution
            .load_run_projection(&continuation_run_id)
            .expect("rehydrated continuation");
        assert_ne!(promoted.status, RunStatus::Queued);
    }
}
