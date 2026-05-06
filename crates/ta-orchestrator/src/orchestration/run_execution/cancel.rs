use std::collections::HashSet;

use ta_protocol::wire::{
    ApprovalActor, ApprovalDecision, ApprovalEvent, ApprovalResolution, DaemonEvent, RunId,
    RunSource, RunStatus,
};
use ta_store::{CommitRunTransition, PersistenceStore, SessionApprovalQuery};

use super::*;

const MAX_NATIVE_CHILD_CANCEL_CASCADE_RUNS: usize = 64;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn cancel_run(
        &self,
        session_id: crate::SessionId,
        actor: ApprovalActor,
        run_id: &RunId,
        reason: Option<String>,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let mut visited = HashSet::new();
        self.cancel_run_with_cascade(session_id, actor, run_id, reason, &mut visited)
    }

    fn cancel_run_with_cascade(
        &self,
        session_id: crate::SessionId,
        actor: ApprovalActor,
        run_id: &RunId,
        reason: Option<String>,
        visited: &mut HashSet<RunId>,
    ) -> Result<RunMutationResult, RunExecutionError> {
        if !visited.insert(run_id.clone()) {
            return Err(RunExecutionError::RunNotCancellable(format!(
                "native child cancel cascade loop at {}",
                run_id.as_str()
            )));
        }
        if visited.len() > MAX_NATIVE_CHILD_CANCEL_CASCADE_RUNS {
            return Err(RunExecutionError::ProviderExecutionFailed(format!(
                "native child cancel cascade exceeded {MAX_NATIVE_CHILD_CANCEL_CASCADE_RUNS} runs"
            )));
        }

        let detail = reason
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Run cancelled")
            .to_string();

        let run_was_running = {
            let store = self.store.lock().expect("app store should not be poisoned");
            let Some(existing_run) = store.run(run_id)? else {
                return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
            };
            if existing_run.session_id != session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    existing_run.id.as_str().to_string(),
                ));
            }
            if !matches!(
                existing_run.status,
                RunStatus::Queued | RunStatus::Running | RunStatus::WaitingForApproval
            ) {
                return Err(RunExecutionError::RunNotCancellable(
                    existing_run.id.as_str().to_string(),
                ));
            }
            let run_was_running = existing_run.status == RunStatus::Running;
            if run_was_running && !self.runtime.is_live_run_running(run_id, &session_id) {
                return Err(RunExecutionError::RunNotCancellable(
                    existing_run.id.as_str().to_string(),
                ));
            }
            if store.session(&session_id)?.is_none() {
                return Err(RunExecutionError::SessionNotFound(
                    session_id.as_str().to_string(),
                ));
            }
            run_was_running
        };
        if run_was_running {
            self.runtime
                .cancel_live_run(run_id, &session_id)
                .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
        }

        let (run, mut events) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let Some(existing_run) = store.run(run_id)? else {
                return Err(RunExecutionError::RunNotFound(run_id.as_str().to_string()));
            };
            if existing_run.session_id != session_id {
                return Err(RunExecutionError::RunSessionMismatch(
                    existing_run.id.as_str().to_string(),
                ));
            }
            if !matches!(
                existing_run.status,
                RunStatus::Queued | RunStatus::Running | RunStatus::WaitingForApproval
            ) {
                return Err(RunExecutionError::RunNotCancellable(
                    existing_run.id.as_str().to_string(),
                ));
            }
            if store.session(&session_id)?.is_none() {
                return Err(RunExecutionError::SessionNotFound(
                    session_id.as_str().to_string(),
                ));
            }

            let mut events = Vec::new();
            if matches!(
                existing_run.status,
                RunStatus::Running | RunStatus::WaitingForApproval
            ) {
                let approvals = store.approvals_for_session(&SessionApprovalQuery {
                    session_id: session_id.clone(),
                    run_id: Some(existing_run.id.clone()),
                    approval_id: None,
                })?;
                events.extend(approvals.into_iter().map(|approval| {
                    let mut resolution = ApprovalResolution::new(
                        approval.id,
                        approval.run_id,
                        ApprovalDecision::Rejected,
                        ta_protocol::wire::ApprovalResolutionReason::Cancelled,
                        actor.clone(),
                        Some(detail.clone()),
                    );
                    if let Some(tool_call_id) = approval.tool_call_id {
                        resolution = resolution.with_tool_call_id(tool_call_id);
                    }
                    DaemonEvent::Approval(ApprovalEvent::Resolved { resolution })
                }));
            }

            let run = RunProjection {
                status: RunStatus::Cancelled,
                ..existing_run
            };
            events.push(DaemonEvent::Run(crate::RunEvent {
                run_id: run.id.clone(),
                status: RunStatus::Cancelled,
                detail: detail.clone(),
                output_contract: None,
                recipe_id: recipe_id_for_run(&run),
                result: None,
            }));
            let committed = store.commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: run.clone(),
                events,
                occurred_at_ms: current_time_ms(),
            })?;
            (committed.run, committed.events)
        };

        let child_run_ids = self.cancellable_native_child_run_ids(&session_id, &run.id, visited)?;
        for child_run_id in child_run_ids {
            let child_cancelled = self.cancel_run_with_cascade(
                session_id.clone(),
                actor.clone(),
                &child_run_id,
                Some(detail.clone()),
                visited,
            )?;
            events.extend(child_cancelled.events);
        }

        let run = project_run_summary(run);
        events.extend(self.advance_ready_queue(&session_id, &run.id, RunStatus::Cancelled)?);

        Ok(RunMutationResult { run, events })
    }

    fn cancellable_native_child_run_ids(
        &self,
        session_id: &crate::SessionId,
        parent_run_id: &RunId,
        visited: &HashSet<RunId>,
    ) -> Result<Vec<RunId>, RunExecutionError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        let mut child_run_ids = store
            .runs()?
            .into_iter()
            .filter_map(|run| {
                if &run.session_id != session_id
                    || &run.id == parent_run_id
                    || visited.contains(&run.id)
                    || !matches!(
                        run.status,
                        RunStatus::Queued | RunStatus::Running | RunStatus::WaitingForApproval
                    )
                {
                    return None;
                }
                match &run.source {
                    RunSource::NativeSubagent {
                        parent_run_id: candidate_parent_run_id,
                        ..
                    } if candidate_parent_run_id == parent_run_id => Some(run.id),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        child_run_ids.sort();
        Ok(child_run_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::run_execution::test_support::*;
    use crate::{DaemonApprovalDecideParams, ListApprovalsQuery, StartRunCommand};
    use ta_protocol::wire::{
        AgentStreamItemId, ApprovalDecision, ApprovalEvent, ApprovalId, ApprovalRequest,
        ApprovalScope, DaemonEvent, RunStatus,
    };
    use taugentic_agent::ExecutionSink;

    #[test]
    fn cancel_run_rejects_terminal_status() {
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
        let started = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "Ship app server hard cut".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("run should start");
        let approval_id = started
            .requested_approval_id()
            .expect("expected approval request event");
        execution
            .decide_approval(
                session.id.clone(),
                approval_actor(),
                DaemonApprovalDecideParams {
                    approval_id,
                    decision: ApprovalDecision::Rejected,
                    commentary: None,
                },
            )
            .expect("approval should reject");

        let error = execution
            .cancel_run(
                session.id.clone(),
                approval_actor(),
                &started.run.id,
                Some("too late".to_string()),
            )
            .expect_err("failed run must not be cancellable");

        assert!(matches!(error, RunExecutionError::RunNotCancellable(_)));
    }

    #[test]
    fn cancel_run_rejects_running_run_without_live_active_owner() {
        let runtime = crate::RuntimeService::bootstrap();
        let (app, execution) = app_and_execution_with_runtime(runtime.clone());
        let session = app
            .open_session(
                TEST_CLIENT_NAME,
                TEST_OWNER_PRINCIPAL_ID,
                &crate::orchestration::OpenSessionRequest {
                    title: "Build daemon app server".to_string(),
                },
            )
            .expect("session should open");

        let started = ensure_running_run(&execution, &session.id, "Ship app server hard cut");
        let running_run_id = started.id.clone();

        assert!(
            runtime
                .run_execution_runtime()
                .release_live_run(&running_run_id)
        );
        let error = execution
            .cancel_run(
                session.id.clone(),
                approval_actor(),
                &running_run_id,
                Some("stop live run".to_string()),
            )
            .expect_err("running run without live owner must not be cancellable");

        assert!(matches!(error, RunExecutionError::RunNotCancellable(_)));
    }

    #[test]
    fn cancel_run_promotes_the_next_queued_run() {
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
                    objective: "Ship active queue owner".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("first run should start");
        let second = execution
            .start_run(
                session.id.clone(),
                StartRunCommand {
                    objective: "Ship promoted queue item".to_string(),
                    ..StartRunCommand::default()
                },
            )
            .expect("second run should queue");

        let cancelled = execution
            .cancel_run(
                session.id.clone(),
                approval_actor(),
                &first.run.id,
                Some("clear active slot".to_string()),
            )
            .expect("cancel should promote queued successor");
        let runs = app.list_runs(&session.id).expect("runs should list");
        let promoted = runs
            .into_iter()
            .find(|run| run.id == second.run.id)
            .expect("queued run should still exist");

        assert_eq!(cancelled.run.status, RunStatus::Cancelled);
        assert!(matches!(
            promoted.status,
            RunStatus::Running | RunStatus::WaitingForApproval
        ));
        assert!(cancelled.events.iter().any(|record| {
            matches!(
                &record.payload,
                DaemonEvent::Run(crate::RunEvent { run_id, status, .. })
                    if *run_id == second.run.id
                        && matches!(status, RunStatus::Running | RunStatus::WaitingForApproval)
            )
        }));
    }

    #[test]
    fn cancel_running_run_resolves_pending_tool_approvals() {
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
        let running = ensure_running_run(&execution, &session.id, "Ship approval cancel bridge");
        attach_noop_handle(&execution, &running.id);
        let tool_call_id = AgentStreamItemId::new("tool-call-cancel").expect("tool call id");
        let requested_at_ms = current_time_ms();
        let ttl = ta_policy::ApprovalTtlPolicy::default();
        let approval = ApprovalRequest::new(
            ApprovalId::new("approval-running-tool").expect("approval id"),
            running.id.clone(),
            ApprovalScope::ProcessExec,
            requested_at_ms,
            ttl.expires_at_ms(requested_at_ms),
            ta_protocol::wire::ApprovalTarget::ToolCall {
                tool_name: "shell".to_string(),
            },
            "tool shell requires approval",
        )
        .expect("approval request")
        .with_tool_call_id(tool_call_id.clone());
        provider_sink(&execution, &session.id, &running.id)
            .request_approval(approval.clone())
            .expect("approval request should persist");

        let pending = app
            .list_approvals(
                &session.id,
                &ListApprovalsQuery {
                    run_id: Some(running.id.clone()),
                    approval_id: None,
                },
            )
            .expect("approvals should list");
        assert_eq!(pending.items.len(), 1);

        let cancelled = execution
            .cancel_run(
                session.id.clone(),
                approval_actor(),
                &running.id,
                Some("turn_interrupted".to_string()),
            )
            .expect("cancel should resolve pending tool approval");

        assert_eq!(cancelled.run.status, RunStatus::Cancelled);
        assert!(cancelled.events.iter().any(|record| {
            matches!(
                &record.payload,
                DaemonEvent::Approval(ApprovalEvent::Resolved { resolution })
                    if resolution.approval_id == approval.id
                        && resolution.tool_call_id.as_ref() == Some(&tool_call_id)
                        && resolution.decision == ApprovalDecision::Rejected
            )
        }));
        let pending_after_cancel = app
            .list_approvals(
                &session.id,
                &ListApprovalsQuery {
                    run_id: Some(running.id),
                    approval_id: None,
                },
            )
            .expect("approvals should list");
        assert!(pending_after_cancel.items.is_empty());
    }
}
