use ta_protocol::wire::{
    ApprovalActor, ApprovalDecision, ApprovalRequest, ApprovalResolution, ApprovalResolutionReason,
    DaemonApprovalDecideParams,
};
use ta_store::CommitRunTransition;

use super::provider_sink::RunCompletionProjection;
use super::*;

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub fn decide_approval(
        &self,
        session_id: crate::SessionId,
        actor: ApprovalActor,
        params: DaemonApprovalDecideParams,
    ) -> Result<RunMutationResult, RunExecutionError> {
        let now_ms = current_time_ms();
        let (mut run, mut events, live_resolution) = {
            let mut store = self.store.lock().expect("app store should not be poisoned");
            let approval = match store.approval_lookup(&session_id, &params.approval_id)? {
                ta_store::SessionApprovalLookup::Pending(approval) => approval,
                ta_store::SessionApprovalLookup::Resolved => {
                    return Err(RunExecutionError::ApprovalAlreadyResolved(
                        params.approval_id.as_str().to_string(),
                    ));
                }
                ta_store::SessionApprovalLookup::NotFound => {
                    return Err(RunExecutionError::ApprovalNotFound(
                        params.approval_id.as_str().to_string(),
                    ));
                }
            };
            if ta_policy::ApprovalTtlPolicy::default().is_expired(approval.expires_at_ms, now_ms) {
                drop(store);
                let (expired_run, expired_events, live_resolution) =
                    self.expire_pending_approval(&session_id, approval, now_ms)?;
                self.publish_records(&expired_events);
                if let Some(resolution) = live_resolution {
                    self.runtime
                        .resolve_live_approval(&expired_run.id, &session_id, resolution)
                        .map_err(map_agent_runtime_error)?;
                }
                return Err(RunExecutionError::ApprovalAlreadyResolved(
                    params.approval_id.as_str().to_string(),
                ));
            }

            let Some(existing_run) = store.run(&approval.run_id)? else {
                return Err(RunExecutionError::RunNotFound(
                    approval.run_id.as_str().to_string(),
                ));
            };
            if store.session(&session_id)?.is_none() {
                return Err(RunExecutionError::SessionNotFound(
                    session_id.as_str().to_string(),
                ));
            }

            let mut resolution = ApprovalResolution::new(
                params.approval_id,
                approval.run_id,
                params.decision,
                ApprovalResolutionReason::User,
                actor,
                params.commentary,
            );
            if let Some(tool_call_id) = approval.tool_call_id {
                resolution = resolution.with_tool_call_id(tool_call_id);
            }

            if existing_run.status == RunStatus::Running
                && self
                    .runtime
                    .is_live_run_running(&existing_run.id, &session_id)
            {
                let committed = store.commit_run_transition(CommitRunTransition {
                    session_id: session_id.clone(),
                    run: existing_run,
                    user_turn: ta_store::UserTurnCommit::NoUserTurn,
                    events: vec![DaemonEvent::Approval(ApprovalEvent::Resolved {
                        resolution: resolution.clone(),
                    })],
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })?;
                (committed.run, committed.events, Some(resolution))
            } else {
                let next_run_status = match params.decision {
                    ApprovalDecision::Approved => RunStatus::Running,
                    ApprovalDecision::Rejected => RunStatus::Failed,
                };

                if existing_run.status != RunStatus::WaitingForApproval {
                    return Err(RunExecutionError::RunNotWaitingForApproval(
                        existing_run.id.as_str().to_string(),
                    ));
                }

                let run = RunProjection {
                    status: next_run_status,
                    ..existing_run
                };
                let run_event = match params.decision {
                    ApprovalDecision::Approved => crate::RunEvent::active(
                        run.id.clone(),
                        run.status,
                        None,
                        recipe_id_for_run(&run),
                        None,
                    )
                    .expect("approved status should be active"),
                    ApprovalDecision::Rejected => crate::RunEvent::terminal(
                        run.id.clone(),
                        run.status,
                        crate::RunStatusReason::new("Approval rejected")
                            .expect("approval rejection reason should be valid"),
                        None,
                        recipe_id_for_run(&run),
                        None,
                    )
                    .expect("rejected status should be terminal"),
                };
                let committed = store.commit_run_transition(CommitRunTransition {
                    session_id: session_id.clone(),
                    run: run.clone(),
                    user_turn: ta_store::UserTurnCommit::NoUserTurn,
                    events: vec![
                        DaemonEvent::Approval(ApprovalEvent::Resolved { resolution }),
                        DaemonEvent::Run(run_event),
                    ],
                    occurred_at_ms: current_time_ms(),
                    auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
                })?;
                (committed.run, committed.events, None)
            }
        };
        let resolved_live_approval = live_resolution.is_some();
        if let Some(resolution) = live_resolution {
            self.runtime
                .resolve_live_approval(&run.id, &session_id, resolution)
                .map_err(map_agent_runtime_error)?;
        }
        if run.status == RunStatus::Running && !resolved_live_approval {
            let generation = self
                .runtime
                .claim_live_run(run.id.clone(), session_id.clone());
            let runtime_profile = self
                .runtime
                .runtime_profile(&run.runtime_profile_id)
                .map_err(map_agent_runtime_error)?;
            let start_result = self.start_provider_execution(
                &session_id,
                &run.id,
                &runtime_profile,
                run.source.route(),
                generation,
            );
            let latest_run = self.load_run_projection(&run.id)?;
            match start_result {
                Ok(()) => {}
                Err(error) if latest_run.status == RunStatus::Running => {
                    let failed = self.commit_failed_live_run_for_generation(
                        session_id.clone(),
                        &latest_run.id,
                        error.to_string(),
                        RunCompletionProjection::default(),
                        generation,
                    )?;
                    run = self.load_run_projection(&latest_run.id)?;
                    events.extend(failed.events);
                }
                Err(_) if latest_run.status != RunStatus::Cancelled => {
                    run = latest_run;
                }
                Err(_) => {}
            }
        }
        if matches!(run.status, RunStatus::Failed) {
            events.extend(self.advance_ready_queue(&session_id, &run.id, RunStatus::Failed)?);
        }
        let run = project_run_summary(run);
        Ok(RunMutationResult { run, events })
    }

    pub(crate) fn expire_pending_approvals_for_session(
        &self,
        session_id: &crate::SessionId,
    ) -> Result<Vec<ta_store::EventRecord>, RunExecutionError> {
        let now_ms = current_time_ms();
        let approvals = {
            let store = self.store.lock().expect("app store should not be poisoned");
            store.approvals_for_session(&ta_store::SessionApprovalQuery {
                session_id: session_id.clone(),
                run_id: None,
                approval_id: None,
            })?
        };
        let mut published = Vec::new();
        for approval in approvals {
            if !ta_policy::ApprovalTtlPolicy::default().is_expired(approval.expires_at_ms, now_ms) {
                continue;
            }
            let (run, events, live_resolution) =
                self.expire_pending_approval(session_id, approval, now_ms)?;
            self.publish_records(&events);
            if let Some(resolution) = live_resolution {
                self.runtime
                    .resolve_live_approval(&run.id, session_id, resolution)
                    .map_err(map_agent_runtime_error)?;
            }
            published.extend(events);
        }
        Ok(published)
    }

    fn expire_pending_approval(
        &self,
        session_id: &crate::SessionId,
        approval: ApprovalRequest,
        now_ms: u64,
    ) -> Result<
        (
            RunProjection,
            Vec<ta_store::EventRecord>,
            Option<ApprovalResolution>,
        ),
        RunExecutionError,
    > {
        let actor = ApprovalActor::new("taugentic-daemon")
            .map_err(|error| RunExecutionError::ProviderExecutionFailed(error.to_string()))?;
        let mut resolution = ApprovalResolution::new(
            approval.id,
            approval.run_id.clone(),
            ApprovalDecision::Rejected,
            ApprovalResolutionReason::Expired,
            actor,
            Some("approval_expired".to_string()),
        );
        if let Some(tool_call_id) = approval.tool_call_id {
            resolution = resolution.with_tool_call_id(tool_call_id);
        }

        let mut store = self.store.lock().expect("app store should not be poisoned");
        let Some(existing_run) = store.run(&approval.run_id)? else {
            return Err(RunExecutionError::RunNotFound(
                approval.run_id.as_str().to_string(),
            ));
        };
        if existing_run.session_id != *session_id {
            return Err(RunExecutionError::RunSessionMismatch(
                existing_run.id.as_str().to_string(),
            ));
        }

        let live_resolution = (existing_run.status == RunStatus::Running
            && self
                .runtime
                .is_live_run_running(&existing_run.id, session_id))
        .then(|| resolution.clone());
        let (run, events) = if existing_run.status == RunStatus::WaitingForApproval {
            let run = RunProjection {
                status: RunStatus::Failed,
                ..existing_run
            };
            let committed = store.commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: run.clone(),
                user_turn: ta_store::UserTurnCommit::NoUserTurn,
                events: vec![
                    DaemonEvent::Approval(ApprovalEvent::Resolved { resolution }),
                    DaemonEvent::Run(
                        crate::RunEvent::terminal(
                            run.id.clone(),
                            run.status,
                            crate::RunStatusReason::new("Approval expired")
                                .expect("approval expiration reason should be valid"),
                            None,
                            recipe_id_for_run(&run),
                            None,
                        )
                        .expect("expired status should be terminal"),
                    ),
                ],
                occurred_at_ms: now_ms,
                auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
            })?;
            (committed.run, committed.events)
        } else {
            let committed = store.commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: existing_run,
                user_turn: ta_store::UserTurnCommit::NoUserTurn,
                events: vec![DaemonEvent::Approval(ApprovalEvent::Resolved {
                    resolution,
                })],
                occurred_at_ms: now_ms,
                auth_profile_mutation: ta_store::AuthProfileCommitMutation::Unchanged,
            })?;
            (committed.run, committed.events)
        };
        Ok((run, events, live_resolution))
    }
}

#[cfg(test)]
mod tests;
