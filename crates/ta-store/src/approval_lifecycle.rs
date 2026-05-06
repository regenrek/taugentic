use std::collections::{BTreeMap, BTreeSet};

use ta_protocol::wire::{ApprovalEvent, ApprovalId, ApprovalRequest, DaemonEvent, RunId};

use crate::{EventRecord, SessionApprovalLookup, SessionApprovalQuery, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApprovalLifecycleState {
    pending: BTreeMap<ApprovalId, (u64, ApprovalRequest)>,
    resolved: BTreeSet<ApprovalId>,
}

impl ApprovalLifecycleState {
    pub(crate) fn fold_session_records<'a>(
        records: impl IntoIterator<Item = &'a EventRecord>,
    ) -> Result<Self, StoreError> {
        let mut state = Self {
            pending: BTreeMap::new(),
            resolved: BTreeSet::new(),
        };
        for record in records {
            state.apply_daemon_event(&record.payload, record.sequence, None)?;
        }
        Ok(state)
    }

    pub(crate) fn approvals_for_query(&self, query: &SessionApprovalQuery) -> Vec<ApprovalRequest> {
        let mut approvals = self
            .pending
            .values()
            .filter_map(|(sequence, approval)| {
                if query
                    .run_id
                    .as_ref()
                    .is_some_and(|run_id| approval.run_id != *run_id)
                {
                    return None;
                }
                if query
                    .approval_id
                    .as_ref()
                    .is_some_and(|approval_id| approval.id != *approval_id)
                {
                    return None;
                }
                Some((*sequence, approval.clone()))
            })
            .collect::<Vec<_>>();
        approvals.sort_by(|left, right| right.0.cmp(&left.0));
        approvals
            .into_iter()
            .map(|(_, approval)| approval)
            .collect()
    }

    pub(crate) fn lookup(&self, approval_id: &ApprovalId) -> SessionApprovalLookup {
        if let Some((_, approval)) = self.pending.get(approval_id) {
            return SessionApprovalLookup::Pending(approval.clone());
        }
        if self.resolved.contains(approval_id) {
            return SessionApprovalLookup::Resolved;
        }
        SessionApprovalLookup::NotFound
    }

    pub(crate) fn validate_run_transition<'a>(
        &self,
        run_id: &RunId,
        events: impl IntoIterator<Item = &'a DaemonEvent>,
    ) -> Result<(), StoreError> {
        let mut state = self.clone();
        for (index, event) in events.into_iter().enumerate() {
            state.apply_daemon_event(event, index as u64, Some(run_id))?;
        }
        Ok(())
    }

    fn apply_daemon_event(
        &mut self,
        event: &DaemonEvent,
        sequence: u64,
        expected_run_id: Option<&RunId>,
    ) -> Result<(), StoreError> {
        let DaemonEvent::Approval(approval_event) = event else {
            return Ok(());
        };
        self.apply_approval_event(approval_event, sequence, expected_run_id)
    }

    fn apply_approval_event(
        &mut self,
        event: &ApprovalEvent,
        sequence: u64,
        expected_run_id: Option<&RunId>,
    ) -> Result<(), StoreError> {
        match event {
            ApprovalEvent::Requested { request } => {
                if expected_run_id.is_some_and(|run_id| request.run_id != *run_id) {
                    return Err(lifecycle_error(
                        &request.id,
                        "approval request run id does not match committed run",
                    ));
                }
                if self.pending.contains_key(&request.id) {
                    return Err(lifecycle_error(
                        &request.id,
                        "approval request is already pending",
                    ));
                }
                if self.resolved.contains(&request.id) {
                    return Err(lifecycle_error(
                        &request.id,
                        "approval request was already resolved",
                    ));
                }
                self.pending
                    .insert(request.id.clone(), (sequence, request.clone()));
                Ok(())
            }
            ApprovalEvent::Resolved { resolution } => {
                if expected_run_id.is_some_and(|run_id| resolution.run_id != *run_id) {
                    return Err(lifecycle_error(
                        &resolution.approval_id,
                        "approval resolution run id does not match committed run",
                    ));
                }
                if self.resolved.contains(&resolution.approval_id) {
                    return Err(lifecycle_error(
                        &resolution.approval_id,
                        "approval is already resolved",
                    ));
                }
                let Some((_, pending_request)) = self.pending.get(&resolution.approval_id) else {
                    return Err(lifecycle_error(
                        &resolution.approval_id,
                        "approval resolution does not match a pending request",
                    ));
                };
                if pending_request.run_id != resolution.run_id {
                    return Err(lifecycle_error(
                        &resolution.approval_id,
                        "approval resolution run id does not match the pending request",
                    ));
                }
                if pending_request.tool_call_id != resolution.tool_call_id {
                    return Err(lifecycle_error(
                        &resolution.approval_id,
                        "approval resolution tool call id does not match the pending request",
                    ));
                }
                self.pending.remove(&resolution.approval_id);
                self.resolved.insert(resolution.approval_id.clone());
                Ok(())
            }
        }
    }
}

fn lifecycle_error(approval_id: &ApprovalId, detail: &str) -> StoreError {
    StoreError::ApprovalLifecycleViolation {
        approval_id: approval_id.as_str().to_string(),
        detail: detail.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use ta_protocol::wire::{
        AgentStreamItemId, ApprovalActor, ApprovalDecision, ApprovalRequest, ApprovalResolution,
        ApprovalResolutionReason, ApprovalScope, ApprovalTarget, SessionId,
    };

    use super::*;

    #[test]
    fn rejects_resolution_without_pending_request() {
        let state = ApprovalLifecycleState::fold_session_records([]).expect("empty state");
        let error = state
            .validate_run_transition(
                &RunId::new("run-1").expect("run id"),
                [DaemonEvent::Approval(ApprovalEvent::Resolved {
                    resolution: ApprovalResolution::new(
                        ApprovalId::new("approval-1").expect("approval id"),
                        RunId::new("run-1").expect("run id"),
                        ApprovalDecision::Approved,
                        ApprovalResolutionReason::User,
                        ApprovalActor::new("principal-test-client").expect("approval actor"),
                        None,
                    ),
                })]
                .iter(),
            )
            .expect_err("orphan resolution must fail");

        assert_eq!(
            error,
            StoreError::ApprovalLifecycleViolation {
                approval_id: "approval-1".to_string(),
                detail: "approval resolution does not match a pending request".to_string(),
            }
        );
    }

    #[test]
    fn rejects_resurrecting_resolved_approval_id() {
        let session_id = SessionId::new("session-1").expect("session id");
        let run_id = RunId::new("run-1").expect("run id");
        let approval_id = ApprovalId::new("approval-1").expect("approval id");
        let request = ApprovalRequest::new(
            approval_id.clone(),
            run_id.clone(),
            ApprovalScope::ProcessExec,
            100,
            200,
            ApprovalTarget::ProcessExec { command: None },
            "execute run executes a process",
        )
        .expect("approval request");
        let resolution = ApprovalResolution::new(
            approval_id.clone(),
            run_id.clone(),
            ApprovalDecision::Approved,
            ApprovalResolutionReason::User,
            ApprovalActor::new("principal-test-client").expect("approval actor"),
            None,
        );
        let records = vec![
            EventRecord {
                sequence: 1,
                session_id: session_id.clone(),
                occurred_at_ms: 10,
                payload: DaemonEvent::Approval(ApprovalEvent::Requested {
                    request: request.clone(),
                }),
            },
            EventRecord {
                sequence: 2,
                session_id,
                occurred_at_ms: 20,
                payload: DaemonEvent::Approval(ApprovalEvent::Resolved { resolution }),
            },
        ];
        let state = ApprovalLifecycleState::fold_session_records(records.iter()).expect("state");

        let error = state
            .validate_run_transition(
                &run_id,
                [DaemonEvent::Approval(ApprovalEvent::Requested { request })].iter(),
            )
            .expect_err("resurrecting approval id must fail");

        assert_eq!(
            error,
            StoreError::ApprovalLifecycleViolation {
                approval_id: "approval-1".to_string(),
                detail: "approval request was already resolved".to_string(),
            }
        );
    }

    #[test]
    fn rejects_resolution_with_mismatched_pending_run_id() {
        let session_id = SessionId::new("session-1").expect("session id");
        let request_run_id = RunId::new("run-1").expect("run id");
        let resolution_run_id = RunId::new("run-2").expect("run id");
        let approval_id = ApprovalId::new("approval-1").expect("approval id");
        let request = ApprovalRequest::new(
            approval_id.clone(),
            request_run_id,
            ApprovalScope::ProcessExec,
            100,
            200,
            ApprovalTarget::ProcessExec { command: None },
            "execute run executes a process",
        )
        .expect("approval request");
        let state = ApprovalLifecycleState::fold_session_records(
            [EventRecord {
                sequence: 1,
                session_id,
                occurred_at_ms: 10,
                payload: DaemonEvent::Approval(ApprovalEvent::Requested { request }),
            }]
            .iter(),
        )
        .expect("state");

        let error = state
            .validate_run_transition(
                &resolution_run_id,
                [DaemonEvent::Approval(ApprovalEvent::Resolved {
                    resolution: ApprovalResolution::new(
                        approval_id,
                        resolution_run_id.clone(),
                        ApprovalDecision::Approved,
                        ApprovalResolutionReason::User,
                        ApprovalActor::new("principal-test-client").expect("approval actor"),
                        None,
                    ),
                })]
                .iter(),
            )
            .expect_err("run mismatch must fail");

        assert_eq!(
            error,
            StoreError::ApprovalLifecycleViolation {
                approval_id: "approval-1".to_string(),
                detail: "approval resolution run id does not match the pending request".to_string(),
            }
        );
    }

    #[test]
    fn rejects_resolution_with_mismatched_pending_tool_call_id() {
        let session_id = SessionId::new("session-1").expect("session id");
        let run_id = RunId::new("run-1").expect("run id");
        let approval_id = ApprovalId::new("approval-1").expect("approval id");
        let request = ApprovalRequest::new(
            approval_id.clone(),
            run_id.clone(),
            ApprovalScope::ProcessExec,
            100,
            200,
            ApprovalTarget::ToolCall {
                tool_name: "shell".to_string(),
            },
            "execute tool requires approval",
        )
        .expect("approval request")
        .with_tool_call_id(AgentStreamItemId::new("tool-call-1").expect("tool call id"));
        let state = ApprovalLifecycleState::fold_session_records(
            [EventRecord {
                sequence: 1,
                session_id,
                occurred_at_ms: 10,
                payload: DaemonEvent::Approval(ApprovalEvent::Requested { request }),
            }]
            .iter(),
        )
        .expect("state");

        let error = state
            .validate_run_transition(
                &run_id,
                [DaemonEvent::Approval(ApprovalEvent::Resolved {
                    resolution: ApprovalResolution::new(
                        approval_id,
                        run_id.clone(),
                        ApprovalDecision::Approved,
                        ApprovalResolutionReason::User,
                        ApprovalActor::new("principal-test-client").expect("approval actor"),
                        None,
                    )
                    .with_tool_call_id(
                        AgentStreamItemId::new("tool-call-2").expect("tool call id"),
                    ),
                })]
                .iter(),
            )
            .expect_err("tool call mismatch must fail");

        assert_eq!(
            error,
            StoreError::ApprovalLifecycleViolation {
                approval_id: "approval-1".to_string(),
                detail: "approval resolution tool call id does not match the pending request"
                    .to_string(),
            }
        );
    }
}
