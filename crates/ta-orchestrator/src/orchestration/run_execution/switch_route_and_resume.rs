use ta_policy::{Operation, evaluate_execution_context};
use ta_protocol::wire::{
    ApprovalScope, RunHarnessKind, RunSource, RunStatus, SwitchRouteAndResumeRequest,
    SwitchRouteAndResumeResult,
};
use ta_store::{
    CommitRunTransition, PersistenceStore, RunEventRangeQuery, RunProjection, UserTurnCommit,
};
use uuid::Uuid;

use super::*;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub fn switch_route_and_resume(
        &self,
        session_id: crate::SessionId,
        request: SwitchRouteAndResumeRequest,
    ) -> Result<SwitchRouteAndResumeResult, RunExecutionError> {
        if request.session_id != session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                request.parent_run_id.as_str().to_string(),
            ));
        }

        let (parent, parent_event_seq) = {
            let store = self.store.lock().expect("app store should not be poisoned");
            let parent = store.run(&request.parent_run_id)?.ok_or_else(|| {
                RunExecutionError::RunNotFound(request.parent_run_id.as_str().to_string())
            })?;
            if parent.session_id != session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    parent.id.as_str().to_string(),
                ));
            }
            if parent.harness != RunHarnessKind::Native
                || parent.status != RunStatus::Failed
                || !matches!(
                    parent.source,
                    RunSource::User { .. }
                        | RunSource::Forked { .. }
                        | RunSource::RouteSwitchedContinuation { .. }
                )
            {
                return Err(RunExecutionError::RunNotRouteExhausted(
                    parent.id.as_str().to_string(),
                ));
            }
            let parent_event_seq = parent.last_event_seq.ok_or_else(|| {
                RunExecutionError::RunNotRouteExhausted(parent.id.as_str().to_string())
            })?;
            let events = store.read_run_events(&RunEventRangeQuery {
                session_id: session_id.clone(),
                run_id: parent.id.clone(),
                after_sequence: parent_event_seq.checked_sub(1),
                limit: 1,
            })?;
            let exhausted = events.records.first().is_some_and(|record| {
                matches!(
                    &record.payload,
                    crate::DaemonEvent::Run(crate::RunEvent::Status(status))
                        if status.status() == RunStatus::Failed
                            && status.auth_profile_exhaustion().is_some()
                )
            });
            if !exhausted {
                return Err(RunExecutionError::RunNotRouteExhausted(
                    parent.id.as_str().to_string(),
                ));
            }
            (parent, parent_event_seq)
        };

        let replacement = self
            .agent_runtime
            .validate_agent_run_selection(&request.selection)
            .map_err(map_agent_runtime_error)?;
        let replacement_route = replacement.route();
        if replacement_route == parent.source.route() {
            return Err(RunExecutionError::ReplacementRouteMustDiffer);
        }

        let child_run_id = crate::RunId::new(format!("run-{}", Uuid::new_v4().simple()))
            .expect("generated run id should be valid");
        let disposition = self
            .runtime
            .schedule_run_start(&session_id, child_run_id.clone())
            .map_err(|error| match error {
                crate::RunSchedulerError::QueueFull(session_id) => {
                    RunExecutionError::RunQueueFull(session_id)
                }
            })?;
        let fail_scheduled_run = |error: RunExecutionError| {
            self.runtime
                .finish_scheduled_run(&session_id, &child_run_id, RunStatus::Failed);
            error
        };
        let decision = evaluate_execution_context(
            &parent.execution_context,
            &Operation::new(
                ApprovalScope::ProcessExec,
                "continue on selected replacement route",
            ),
        );

        let (mut run, events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let (status, events) = match disposition {
                crate::RunScheduleDisposition::StartNow => {
                    build_start_transition(child_run_id.clone(), decision, None)
                }
                crate::RunScheduleDisposition::Queued { position } => {
                    build_queue_transition(child_run_id.clone(), position, None)
                }
            };
            let child = RunProjection {
                id: child_run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: replacement.runtime_profile().id.clone(),
                objective: parent.objective.clone(),
                status,
                harness: run_harness_kind(replacement.execution_harness()),
                source: RunSource::RouteSwitchedContinuation {
                    route: replacement_route.clone(),
                    parent_run_id: parent.id.clone(),
                    parent_event_seq,
                },
                execution_context: parent.execution_context.clone(),
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: parent.workspace_info.clone(),
                claimed_files: parent.claimed_files.clone(),
                conflict_summary: parent.conflict_summary.clone(),
            };
            let committed = store
                .commit_run_transition(CommitRunTransition {
                    session_id: session_id.clone(),
                    run: child,
                    // The failed parent's original durable User turn is the
                    // sole owner of this objective; replay is ephemeral.
                    user_turn: UserTurnCommit::NoUserTurn,
                    events,
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })
                .map_err(|error| fail_scheduled_run(RunExecutionError::Store(error)))?;
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
                replacement.runtime_profile(),
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
        Ok(SwitchRouteAndResumeResult {
            run: project_run_record(run),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::AppService;
    use std::sync::Arc;
    use ta_protocol::wire::{AgentRuntimeSelection, AuthProfileExhaustion, AuthProfileId};
    use ta_store::{AuthProfileRepository, EventLogRepository};
    use taugentic_agent::{ExecutionError, ExecutionSink};

    #[test]
    fn typed_exhaustion_creates_an_immutable_generic_route_successor() {
        let (runtime, _dispatcher) =
            runtime_with_dispatch_plans([DispatchPlan::Succeed(Arc::new(NoopExecutionHandle))]);
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Switch route and resume");
        let parent_id = start_production_shaped_running_run(
            &app,
            &execution,
            &session.id,
            "repeat this exact text",
        );
        let parent_before = execution.load_run_projection(&parent_id).expect("parent");
        let previous_route = parent_before.source.route().clone();
        let replacement = crate::orchestration::test_runtime_selection(&app, "runtime-codex-safe");
        provider_sink(&execution, &session.id, &parent_id)
            .fail(ExecutionError::CreditsExhausted("redacted".to_string()))
            .expect("typed exhaustion");

        let switched = execution
            .switch_route_and_resume(
                session.id.clone(),
                SwitchRouteAndResumeRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent_id.clone(),
                    selection: replacement.clone(),
                },
            )
            .expect("switch should create a child");
        let parent_after = execution.load_run_projection(&parent_id).expect("parent");
        let child = execution
            .load_run_projection(&switched.run.id)
            .expect("child");

        assert_ne!(switched.run.status, RunStatus::Queued);
        assert_eq!(parent_after.status, RunStatus::Failed);
        assert_eq!(parent_after.objective, parent_before.objective);
        assert_eq!(parent_after.source, parent_before.source);
        assert_eq!(
            parent_after.execution_context,
            parent_before.execution_context
        );
        assert_eq!(parent_after.workspace_info, parent_before.workspace_info);
        assert_eq!(parent_after.claimed_files, parent_before.claimed_files);
        assert_eq!(
            parent_after.conflict_summary,
            parent_before.conflict_summary
        );
        assert_ne!(child.id, parent_id);
        assert_eq!(child.execution_context, parent_before.execution_context);
        assert_eq!(child.workspace_info, parent_before.workspace_info);
        assert_eq!(child.claimed_files, parent_before.claimed_files);
        assert_eq!(child.conflict_summary, parent_before.conflict_summary);
        match &child.source {
            RunSource::RouteSwitchedContinuation {
                route,
                parent_run_id,
                parent_event_seq,
            } => {
                assert_eq!(parent_run_id, &parent_id);
                assert_eq!(route.auth_profile_id, replacement.auth_profile_id);
                assert_eq!(route.runtime_profile_id, replacement.runtime_profile_id);
                assert_eq!(route.model_id, replacement.model_id);
                assert_ne!(route.provider_id, previous_route.provider_id);
                assert_ne!(route.harness, previous_route.harness);
                assert_eq!(
                    *parent_event_seq,
                    parent_after.last_event_seq.expect("terminal event")
                );
            }
            source => panic!("unexpected child source: {source:?}"),
        }
        assert_eq!(
            execution
                .store
                .lock()
                .expect("store")
                .auth_profile(&previous_route.auth_profile_id.expect("parent profile"))
                .expect("profile")
                .expect("profile")
                .profile
                .exhaustion,
            Some(AuthProfileExhaustion::CreditsExhausted)
        );
        assert_eq!(
            durable_user_turns_for_run(&execution, &session.id, &parent_id),
            vec!["repeat this exact text"]
        );
        assert!(durable_user_turns_for_run(&execution, &session.id, &child.id).is_empty());
        let history = execution
            .native_history_initial_state_for_run(&session.id, &child.id)
            .expect("history should build")
            .expect("route-switched child uses native history");
        assert_eq!(
            history.objective_policy,
            taugentic_agent::NativeHistoryObjectivePolicy::ObjectiveAlreadyInHistory
        );
        assert_eq!(
            history
                .messages
                .iter()
                .filter(|message| message.content == parent_before.objective)
                .count(),
            1
        );
    }

    #[test]
    fn unexhausted_route_rejects_without_creating_a_child() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "No unsafe route switch");
        let parent = ensure_running_run(&app, &execution, &session.id, "do not mutate");
        let route = execution
            .load_run_projection(&parent.id)
            .expect("parent")
            .source
            .route()
            .clone();
        let before = execution
            .store
            .lock()
            .expect("store")
            .events_for_session(&session.id)
            .expect("events")
            .len();
        let error = execution
            .switch_route_and_resume(
                session.id.clone(),
                SwitchRouteAndResumeRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent.id,
                    selection: AgentRuntimeSelection {
                        runtime_profile_id: route.runtime_profile_id,
                        auth_profile_id: route.auth_profile_id,
                        model_id: route.model_id,
                    },
                },
            )
            .expect_err("non-terminal run must reject");
        assert!(matches!(error, RunExecutionError::RunNotRouteExhausted(_)));
        assert_eq!(
            execution
                .store
                .lock()
                .expect("store")
                .events_for_session(&session.id)
                .expect("events")
                .len(),
            before
        );
    }

    #[test]
    fn typed_exhaustion_with_equal_route_rejects_without_scheduling_or_durable_mutation() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Equal route must not continue");
        let parent = ensure_running_run(&app, &execution, &session.id, "keep parent unchanged");
        let route = execution
            .load_run_projection(&parent.id)
            .expect("parent")
            .source
            .route()
            .clone();
        provider_sink(&execution, &session.id, &parent.id)
            .fail(ExecutionError::CreditsExhausted("redacted".to_string()))
            .expect("typed exhaustion");
        let before_parent = execution.load_run_projection(&parent.id).expect("parent");
        let before_runs = app.list_runs(&session.id).expect("runs");
        let before_events = execution
            .store
            .lock()
            .expect("store")
            .events_for_session(&session.id)
            .expect("events")
            .len();

        let error = execution
            .switch_route_and_resume(
                session.id.clone(),
                SwitchRouteAndResumeRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent.id,
                    selection: AgentRuntimeSelection {
                        runtime_profile_id: route.runtime_profile_id,
                        auth_profile_id: route.auth_profile_id,
                        model_id: route.model_id,
                    },
                },
            )
            .expect_err("equal route must reject before scheduling");

        assert!(matches!(
            error,
            RunExecutionError::ReplacementRouteMustDiffer
        ));
        assert_eq!(
            execution
                .load_run_projection(&before_parent.id)
                .expect("parent"),
            before_parent
        );
        assert_eq!(app.list_runs(&session.id).expect("runs"), before_runs);
        assert_eq!(
            execution
                .store
                .lock()
                .expect("store")
                .events_for_session(&session.id)
                .expect("events")
                .len(),
            before_events
        );
    }

    #[test]
    fn typed_invalid_replacement_after_exhaustion_does_not_mutate_durable_state() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Invalid replacement does not mutate");
        let parent = ensure_running_run(&app, &execution, &session.id, "keep this durable state");
        let route = execution
            .load_run_projection(&parent.id)
            .expect("parent")
            .source
            .route()
            .clone();
        let replacement = AuthProfileId::new("profile-invalid-replacement").expect("replacement");
        {
            let mut store = execution.store.lock().expect("store");
            let auth_method_id = store
                .auth_profile(route.auth_profile_id.as_ref().expect("parent profile"))
                .expect("profile")
                .expect("profile")
                .auth_method_id()
                .clone();
            store
                .save_auth_profile(ta_store::connected_test_auth_profile(
                    replacement.as_str(),
                    auth_method_id.as_str(),
                    route.provider_id.as_str(),
                ))
                .expect("replacement profile");
        }
        provider_sink(&execution, &session.id, &parent.id)
            .fail(ExecutionError::CreditsExhausted("redacted".to_string()))
            .expect("typed exhaustion");
        let before_parent = execution.load_run_projection(&parent.id).expect("parent");
        let before_events = execution
            .store
            .lock()
            .expect("store")
            .events_for_session(&session.id)
            .expect("events")
            .len();

        let error = execution
            .switch_route_and_resume(
                session.id.clone(),
                SwitchRouteAndResumeRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent.id.clone(),
                    selection: AgentRuntimeSelection {
                        runtime_profile_id: route.runtime_profile_id,
                        auth_profile_id: Some(replacement),
                        model_id: Some(
                            ta_protocol::wire::AgentRuntimeModelId::new("not-the-parent-model")
                                .expect("typed model id"),
                        ),
                    },
                },
            )
            .expect_err("mismatched explicit selection must reject");
        assert!(matches!(
            error,
            RunExecutionError::ProviderExecutionFailed(_)
        ));
        assert_eq!(
            execution.load_run_projection(&parent.id).expect("parent"),
            before_parent
        );
        assert_eq!(
            execution
                .store
                .lock()
                .expect("store")
                .events_for_session(&session.id)
                .expect("events")
                .len(),
            before_events
        );
    }

    #[test]
    fn queued_route_switch_rehydrates_with_the_same_canonical_history() {
        let (runtime, _dispatcher) =
            runtime_with_dispatch_plans([DispatchPlan::Succeed(Arc::new(NoopExecutionHandle))]);
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Queued route switch rehydration");
        let parent_id =
            start_production_shaped_running_run(&app, &execution, &session.id, "replay this once");
        let route = execution
            .load_run_projection(&parent_id)
            .expect("parent")
            .source
            .route()
            .clone();
        let replacement = AuthProfileId::new("profile-queued-replacement").expect("replacement");
        {
            let mut store = execution.store.lock().expect("store");
            let auth_method_id = store
                .auth_profile(route.auth_profile_id.as_ref().expect("parent profile"))
                .expect("profile")
                .expect("profile")
                .auth_method_id()
                .clone();
            store
                .save_auth_profile(ta_store::connected_test_auth_profile(
                    replacement.as_str(),
                    auth_method_id.as_str(),
                    route.provider_id.as_str(),
                ))
                .expect("replacement profile");
        }
        provider_sink(&execution, &session.id, &parent_id)
            .fail(ExecutionError::CreditsExhausted("redacted".to_string()))
            .expect("typed exhaustion");
        let blocker = ensure_running_run(&app, &execution, &session.id, "occupy queue slot");
        let switched = execution
            .switch_route_and_resume(
                session.id.clone(),
                SwitchRouteAndResumeRequest {
                    session_id: session.id.clone(),
                    parent_run_id: parent_id.clone(),
                    selection: AgentRuntimeSelection {
                        runtime_profile_id: route.runtime_profile_id,
                        auth_profile_id: Some(replacement),
                        model_id: route.model_id,
                    },
                },
            )
            .expect("queued replacement child");
        assert_eq!(switched.run.status, RunStatus::Queued);
        assert_eq!(
            durable_user_turns_for_run(&execution, &session.id, &parent_id),
            vec!["replay this once"]
        );
        assert!(durable_user_turns_for_run(&execution, &session.id, &switched.run.id).is_empty());
        let before_restart = execution
            .native_history_initial_state_for_run(&session.id, &switched.run.id)
            .expect("canonical history")
            .expect("native history");
        assert_eq!(
            before_restart.objective_policy,
            taugentic_agent::NativeHistoryObjectivePolicy::ObjectiveAlreadyInHistory
        );
        assert_eq!(
            before_restart
                .messages
                .iter()
                .filter(|message| message.content == "replay this once")
                .count(),
            1
        );

        provider_sink(&execution, &session.id, &blocker.id)
            .complete("queue blocker completed")
            .expect("blocker completion");
        let restarted_runtime = crate::RuntimeService::bootstrap();
        let restarted = AppService::from_runtime(execution.store.clone(), &restarted_runtime);
        restarted
            .rehydrate_run_scheduler_on_boot()
            .expect("scheduler rehydration");
        let after_restart = restarted
            .run_execution
            .native_history_initial_state_for_run(&session.id, &switched.run.id)
            .expect("rehydrated canonical history")
            .expect("native history");
        assert_eq!(after_restart, before_restart);
        assert_eq!(
            durable_user_turns_for_run(&restarted.run_execution, &session.id, &parent_id),
            vec!["replay this once"]
        );
        assert!(
            durable_user_turns_for_run(&restarted.run_execution, &session.id, &switched.run.id)
                .is_empty()
        );
        assert_eq!(
            after_restart
                .messages
                .iter()
                .filter(|message| message.content == "replay this once")
                .count(),
            1
        );
        assert_ne!(
            restarted
                .run_execution
                .load_run_projection(&switched.run.id)
                .expect("rehydrated child")
                .status,
            RunStatus::Queued
        );
    }
}
