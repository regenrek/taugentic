use ta_policy::Operation;
use ta_protocol::wire::{ApprovalScope, RunHarnessKind, RunSource, RunStatus};
use ta_store::{CommitRunTransition, PersistenceStore, RunProjection};
use taugentic_agent::{AgentExecutionHarness, NativeChildRunRequest, NativeChildRunResult};
use uuid::Uuid;

use super::*;
use crate::{DelegateRecipeResolutionRequest, resolve_delegate_recipe};

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub fn start_native_child_run(
        &self,
        session_id: crate::SessionId,
        request: NativeChildRunRequest,
    ) -> Result<NativeChildRunResult, RunExecutionError> {
        let resolved_request = resolve_delegate_recipe(
            &self.recipe_registry,
            DelegateRecipeResolutionRequest {
                objective: request.objective,
                output_contract: request.output_contract,
                model_id: request.model_id,
                sandbox_profile: request.sandbox_profile,
                recipe_id: request.recipe_id,
            },
        )
        .map_err(map_recipe_resolution_error)?;
        let objective = resolved_request.objective.trim();
        if objective.is_empty() {
            return Err(RunExecutionError::EmptyRunObjective);
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
            if parent.status != RunStatus::Running
                || !self
                    .runtime
                    .is_live_run_running(&parent.id, &parent.session_id)
            {
                return Err(RunExecutionError::RunNotLiveOwned(
                    parent.id.as_str().to_string(),
                ));
            }
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

        let child_run_id = crate::RunId::new(format!("run-{}", Uuid::new_v4().simple()))
            .expect("generated run id should be valid");
        let scheduling_policy = match request.workspace_scope {
            crate::WorkspaceMode::WorktreeWrite => crate::RunSchedulingPolicy::ParallelIfBusy,
            _ => crate::RunSchedulingPolicy::QueueIfBusy,
        };
        let disposition = self
            .runtime
            .schedule_run_start_with_policy(&session_id, child_run_id.clone(), scheduling_policy)
            .map_err(|error| match error {
                crate::RunSchedulerError::QueueFull(session_id) => {
                    RunExecutionError::RunQueueFull(session_id)
                }
            })?;
        let fail_scheduled_run = |error| {
            self.runtime
                .finish_scheduled_run(&session_id, &child_run_id, RunStatus::Failed);
            error
        };
        let operation = Operation::new(ApprovalScope::ProcessExec, "execute native child run");
        let decision = self
            .runtime
            .evaluate_operation_for_policy_mode(&operation, runtime_profile.policy_mode);
        let prepared_context = self
            .prepare_execution_context(
                &session_id,
                &child_run_id,
                &runtime_profile,
                ExecutionContextRequest {
                    workspace_mode: request.workspace_scope,
                    cleanup_policy: request.cleanup_policy,
                    planned_write_files: request.planned_write_files.clone(),
                },
            )
            .map_err(&fail_scheduled_run)?;
        let execution_harness = self
            .runtime
            .execution_harness_for_runtime_profile(&runtime_profile)
            .map_err(map_agent_runtime_error)
            .map_err(&fail_scheduled_run)?;
        if !matches!(execution_harness, AgentExecutionHarness::NativeLoop) {
            return Err(fail_scheduled_run(RunExecutionError::RunNotNativeHarness(
                parent.id.as_str().to_string(),
            )));
        }
        let conflict_event = prepared_context.conflict_warning.clone().map(|warning| {
            DaemonEvent::Conflict(crate::ConflictEvent::Warning {
                run_id: child_run_id.clone(),
                warning,
            })
        });
        let (mut run, events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let (status, mut events) = match disposition {
                crate::RunScheduleDisposition::StartNow => build_start_transition(
                    child_run_id.clone(),
                    decision,
                    resolved_request.recipe_id.clone(),
                ),
                crate::RunScheduleDisposition::Queued { position } => build_queue_transition(
                    child_run_id.clone(),
                    position,
                    resolved_request.recipe_id.clone(),
                ),
            };
            if let Some(conflict_event) = conflict_event {
                events.push(conflict_event);
            }
            let child = RunProjection {
                id: child_run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: runtime_profile.id.clone(),
                objective: objective.to_string(),
                status,
                harness: RunHarnessKind::Native,
                source: RunSource::NativeSubagent {
                    parent_run_id: parent.id.clone(),
                    parent_turn_id: request.parent_turn_id,
                    output_contract: resolved_request.output_contract,
                    model_id: resolved_request.model_id.clone(),
                    sandbox_profile: resolved_request.sandbox_profile.clone(),
                    recipe_id: resolved_request.recipe_id.clone(),
                    workspace_scope: request.workspace_scope,
                    cleanup_policy: request.cleanup_policy,
                    planned_write_files: request.planned_write_files.clone(),
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
                    run: child,
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
        self.publish_records(&events);

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
                self.fail_live_run_and_publish(
                    session_id.clone(),
                    &latest_run.id,
                    error.to_string(),
                )?;
                run = self.load_run_projection(&latest_run.id)?;
            } else if latest_run.status != RunStatus::Cancelled {
                run = latest_run;
            }
        }

        Ok(NativeChildRunResult {
            run_id: run.id,
            status: run.status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ListApprovalsQuery, RunId, orchestration::run_execution::test_support::*};
    use ta_protocol::wire::{
        ApprovalDecision, ApprovalEvent, ApprovalId, ApprovalRequest, ApprovalScope, DaemonEvent,
        RunSource, RunStatus,
    };
    use ta_store::{CommitRepository, ProjectionRepository};
    use taugentic_agent::ExecutionSink;

    #[test]
    fn start_native_child_run_records_parent_lineage_and_inherits_profile() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Native parent");
        select_runtime_profile(&app, "runtime-openai-safe");
        let parent = execution
            .seed_running_run_for_tests(session.id.clone(), "Parent native run".to_string())
            .expect("parent should seed");
        let parent_turn_id =
            ta_protocol::wire::AgentStreamTurnId::new("turn-parent").expect("turn id");

        let child = execution
            .start_native_child_run(
                session.id.clone(),
                NativeChildRunRequest::new(
                    parent.run.id.clone(),
                    parent_turn_id.clone(),
                    "Review the focused files",
                    None,
                    None,
                    None,
                    None,
                )
                .expect("child request"),
            )
            .expect("native child run should start through orchestrator contract");

        assert_eq!(child.status, RunStatus::Queued);
        let stored_child = execution
            .store
            .lock()
            .expect("store should not poison")
            .run(&child.run_id)
            .expect("child lookup should work")
            .expect("child run should persist");
        assert_eq!(
            stored_child.runtime_profile_id,
            parent.run.runtime_profile_id
        );
        assert_eq!(stored_child.objective, "Review the focused files");
        assert_eq!(
            stored_child.source,
            RunSource::NativeSubagent {
                parent_run_id: parent.run.id,
                parent_turn_id,
                output_contract: None,
                model_id: None,
                sandbox_profile: None,
                recipe_id: None,
                workspace_scope: crate::WorkspaceMode::WorkspaceWrite,
                cleanup_policy: crate::WorktreeCleanupPolicy::DeleteOnSuccess,
                planned_write_files: Vec::new(),
            }
        );
    }

    #[test]
    fn queued_native_child_promotion_preserves_stored_runtime_policy() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Native parent");
        select_runtime_profile(&app, "runtime-openai-safe");
        let parent = execution
            .seed_running_run_for_tests(session.id.clone(), "Parent native run".to_string())
            .expect("parent should seed");
        let child = execution
            .start_native_child_run(
                session.id.clone(),
                NativeChildRunRequest::new(
                    parent.run.id.clone(),
                    ta_protocol::wire::AgentStreamTurnId::new("turn-parent").expect("turn id"),
                    "Review the focused files",
                    None,
                    None,
                    None,
                    None,
                )
                .expect("child request"),
            )
            .expect("native child run should queue");
        assert_eq!(child.status, RunStatus::Queued);

        select_runtime_profile(&app, "runtime-openai-allow");
        ProviderRunExecutionSink {
            service: execution.clone(),
            session_id: session.id.clone(),
            run_id: parent.run.id.clone(),
        }
        .complete("parent completed")
        .expect("parent completion should promote queued child");

        let promoted_child = execution
            .store
            .lock()
            .expect("store should not poison")
            .run(&child.run_id)
            .expect("child lookup should work")
            .expect("child run should persist");
        let approvals = app
            .list_approvals(
                &session.id,
                &ListApprovalsQuery {
                    run_id: Some(child.run_id),
                    approval_id: None,
                },
            )
            .expect("approvals should list");

        assert_eq!(promoted_child.status, RunStatus::WaitingForApproval);
        assert_eq!(
            promoted_child.runtime_profile_id.as_str(),
            "runtime-openai-safe"
        );
        assert_eq!(approvals.items.len(), 1);
    }

    #[test]
    fn start_native_child_run_rejects_missing_parent() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Native parent");

        let error = execution
            .start_native_child_run(
                session.id.clone(),
                NativeChildRunRequest::new(
                    RunId::new("run-missing").expect("run id"),
                    ta_protocol::wire::AgentStreamTurnId::new("turn-parent").expect("turn id"),
                    "Review the focused files",
                    None,
                    None,
                    None,
                    None,
                )
                .expect("child request"),
            )
            .expect_err("missing parent must fail");

        assert!(matches!(
            error,
            RunExecutionError::RunNotFound(ref run_id) if run_id == "run-missing"
        ));
    }

    #[test]
    fn start_native_child_run_rejects_external_parent_harness() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "External parent");
        select_runtime_profile(&app, "runtime-codex-safe");
        let parent = execution
            .seed_running_run_for_tests(session.id.clone(), "Parent external run".to_string())
            .expect("parent should seed");

        let error = execution
            .start_native_child_run(
                session.id.clone(),
                NativeChildRunRequest::new(
                    parent.run.id.clone(),
                    ta_protocol::wire::AgentStreamTurnId::new("turn-parent").expect("turn id"),
                    "Review the focused files",
                    None,
                    None,
                    None,
                    None,
                )
                .expect("child request"),
            )
            .expect_err("external parent must fail");

        assert!(matches!(
            error,
            RunExecutionError::RunNotNativeHarness(ref run_id) if run_id == parent.run.id.as_str()
        ));
    }

    #[test]
    fn cancel_run_cascades_to_native_child_runs() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime);
        let session = open_session(&app, "Native parent");
        select_runtime_profile(&app, "runtime-openai-safe");
        let parent = execution
            .seed_running_run_for_tests(session.id.clone(), "Parent native run".to_string())
            .expect("parent should seed");
        attach_noop_handle(&execution, &parent.run.id);
        let queued_child = execution
            .start_native_child_run(
                session.id.clone(),
                NativeChildRunRequest::new(
                    parent.run.id.clone(),
                    ta_protocol::wire::AgentStreamTurnId::new("turn-queued").expect("turn id"),
                    "Queued child",
                    None,
                    None,
                    None,
                    None,
                )
                .expect("child request"),
            )
            .expect("queued native child should start");
        let running_child_id = RunId::new("run-native-child-running").expect("run id");
        let waiting_child_id = RunId::new("run-native-child-waiting").expect("run id");
        let requested_at_ms = current_time_ms();
        let ttl = ta_policy::ApprovalTtlPolicy::default();
        let waiting_approval = ApprovalRequest::new(
            ApprovalId::new("approval-native-child-waiting").expect("approval id"),
            waiting_child_id.clone(),
            ApprovalScope::ProcessExec,
            requested_at_ms,
            ttl.expires_at_ms(requested_at_ms),
            ta_protocol::wire::ApprovalTarget::CapsuleDispatch {
                child_run_id: Some(waiting_child_id.clone()),
                workspace_scope: None,
            },
            "waiting native child requires approval",
        )
        .expect("approval request");
        {
            let mut store = execution
                .store
                .lock()
                .expect("app store should not be poisoned");
            store
                .commit_run_transition(CommitRunTransition {
                    session_id: session.id.clone(),
                    run: RunProjection {
                        id: running_child_id.clone(),
                        session_id: session.id.clone(),
                        runtime_profile_id: parent.run.runtime_profile_id.clone(),
                        objective: "Running child".to_string(),
                        status: RunStatus::Running,
                        harness: RunHarnessKind::Native,
                        source: RunSource::NativeSubagent {
                            parent_run_id: parent.run.id.clone(),
                            parent_turn_id: ta_protocol::wire::AgentStreamTurnId::new(
                                "turn-running",
                            )
                            .expect("turn id"),
                            output_contract: None,
                            model_id: None,
                            sandbox_profile: None,
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
                    events: vec![DaemonEvent::Run(crate::RunEvent {
                        run_id: running_child_id.clone(),
                        status: RunStatus::Running,
                        detail: "Seeded running native child".to_string(),
                        output_contract: None,
                        recipe_id: None,
                        result: None,
                    })],
                    occurred_at_ms: current_time_ms(),
                })
                .expect("running child should persist");
            store
                .commit_run_transition(CommitRunTransition {
                    session_id: session.id.clone(),
                    run: RunProjection {
                        id: waiting_child_id.clone(),
                        session_id: session.id.clone(),
                        runtime_profile_id: parent.run.runtime_profile_id.clone(),
                        objective: "Waiting child".to_string(),
                        status: RunStatus::WaitingForApproval,
                        harness: RunHarnessKind::Native,
                        source: RunSource::NativeSubagent {
                            parent_run_id: parent.run.id.clone(),
                            parent_turn_id: ta_protocol::wire::AgentStreamTurnId::new(
                                "turn-waiting",
                            )
                            .expect("turn id"),
                            output_contract: None,
                            model_id: None,
                            sandbox_profile: None,
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
                    events: vec![
                        DaemonEvent::Approval(ApprovalEvent::Requested {
                            request: waiting_approval.clone(),
                        }),
                        DaemonEvent::Run(crate::RunEvent {
                            run_id: waiting_child_id.clone(),
                            status: RunStatus::WaitingForApproval,
                            detail: "Seeded waiting native child".to_string(),
                            output_contract: None,
                            recipe_id: None,
                            result: None,
                        }),
                    ],
                    occurred_at_ms: current_time_ms(),
                })
                .expect("waiting child should persist");
        }
        execution
            .runtime
            .claim_live_run(running_child_id.clone(), session.id.clone());
        attach_noop_handle(&execution, &running_child_id);

        let pending_before_cancel = app
            .list_approvals(
                &session.id,
                &ListApprovalsQuery {
                    run_id: Some(waiting_child_id.clone()),
                    approval_id: None,
                },
            )
            .expect("approvals should list");
        assert_eq!(pending_before_cancel.items.len(), 1);

        let cancelled = execution
            .cancel_run(
                session.id.clone(),
                approval_actor(),
                &parent.run.id,
                Some("cancel parent".to_string()),
            )
            .expect("parent cancel should cascade to native children");
        let store = execution.store.lock().expect("store should not poison");
        let queued_child = store
            .run(&queued_child.run_id)
            .expect("queued child lookup should work")
            .expect("queued child should persist");
        let running_child = store
            .run(&running_child_id)
            .expect("running child lookup should work")
            .expect("running child should persist");
        let waiting_child = store
            .run(&waiting_child_id)
            .expect("waiting child lookup should work")
            .expect("waiting child should persist");
        drop(store);
        let pending_after_cancel = app
            .list_approvals(
                &session.id,
                &ListApprovalsQuery {
                    run_id: Some(waiting_child_id.clone()),
                    approval_id: None,
                },
            )
            .expect("approvals should list");

        assert_eq!(cancelled.run.status, RunStatus::Cancelled);
        assert_eq!(queued_child.status, RunStatus::Cancelled);
        assert_eq!(running_child.status, RunStatus::Cancelled);
        assert_eq!(waiting_child.status, RunStatus::Cancelled);
        assert!(cancelled.events.iter().any(|record| {
            matches!(
                &record.payload,
                DaemonEvent::Run(crate::RunEvent { run_id, status, .. })
                    if *run_id == queued_child.id && *status == RunStatus::Cancelled
            )
        }));
        assert!(cancelled.events.iter().any(|record| {
            matches!(
                &record.payload,
                DaemonEvent::Run(crate::RunEvent { run_id, status, .. })
                    if *run_id == running_child_id && *status == RunStatus::Cancelled
            )
        }));
        assert!(cancelled.events.iter().any(|record| {
            matches!(
                &record.payload,
                DaemonEvent::Run(crate::RunEvent { run_id, status, .. })
                    if *run_id == waiting_child_id && *status == RunStatus::Cancelled
            )
        }));
        assert!(cancelled.events.iter().any(|record| {
            matches!(
                &record.payload,
                DaemonEvent::Approval(ApprovalEvent::Resolved { resolution })
                    if resolution.approval_id == waiting_approval.id
                        && resolution.decision == ApprovalDecision::Rejected
            )
        }));
        assert!(pending_after_cancel.items.is_empty());
    }
}
