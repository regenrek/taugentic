use super::*;
use crate::DaemonApprovalDecideParams;
use crate::orchestration::run_execution::test_support::*;
use ta_protocol::wire::{
    AgentStreamItemId, ApprovalDecision, ApprovalEvent, ApprovalId, ApprovalRequest,
    ApprovalResolutionReason, ApprovalScope, ApprovalTarget, DaemonEvent, RunHarnessKind, RunId,
    RunSource, RunStatus,
};
use ta_store::{CommitRepository, EventLogRepository};
use taugentic_agent::ExecutionSink;

#[test]
fn decide_approval_rejects_already_resolved_request() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &crate::orchestration::OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let started = execution
        .start_run(
            session.id.clone(),
            start_run_command(&app, "Ship app server hard cut", "runtime-openai-safe"),
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
                approval_id: approval_id.clone(),
                decision: ApprovalDecision::Approved,
                commentary: None,
            },
        )
        .expect("first decision should succeed");

    let error = execution
        .decide_approval(
            session.id.clone(),
            approval_actor(),
            DaemonApprovalDecideParams {
                approval_id,
                decision: ApprovalDecision::Rejected,
                commentary: None,
            },
        )
        .expect_err("second decision must fail");

    assert!(matches!(
        error,
        RunExecutionError::ApprovalAlreadyResolved(_)
    ));
}

#[test]
fn decide_approval_rejects_missing_request() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &crate::orchestration::OpenSessionRequest {
                title: "Build daemon app server".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");

    let error = execution
        .decide_approval(
            session.id.clone(),
            approval_actor(),
            DaemonApprovalDecideParams {
                approval_id: ta_protocol::wire::ApprovalId::new("approval-missing")
                    .expect("approval id"),
                decision: ApprovalDecision::Approved,
                commentary: None,
            },
        )
        .expect_err("missing approval must fail");

    assert!(matches!(error, RunExecutionError::ApprovalNotFound(_)));
}

#[test]
fn list_approvals_expires_stale_waiting_request() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &crate::orchestration::OpenSessionRequest {
                title: "Expire stale approval".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let run_id = RunId::new("run-expired-approval").expect("run id");
    let approval_id = ApprovalId::new("approval-expired").expect("approval id");
    let request = ApprovalRequest::new(
        approval_id.clone(),
        run_id.clone(),
        ApprovalScope::ProcessExec,
        1,
        2,
        ApprovalTarget::ProcessExec { command: None },
        "execute run requires approval",
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
                    id: run_id.clone(),
                    session_id: session.id.clone(),
                    runtime_profile_id: crate::RuntimeProfileId::new("runtime-openai-safe")
                        .expect("runtime profile id"),
                    objective: "Expire stale approval".to_string(),
                    status: RunStatus::WaitingForApproval,
                    harness: RunHarnessKind::Native,
                    source: RunSource::User {
                        route: ta_store::default_test_run_source().route().clone(),
                        output_contract: None,
                        model_id: None,
                        recipe_id: None,
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
                    DaemonEvent::Run(crate::RunEvent {
                        run_id: run_id.clone(),
                        status: RunStatus::WaitingForApproval,
                        detail: "Waiting for approval".to_string(),
                        output_contract: None,
                        recipe_id: None,
                        result: None,
                    }),
                    DaemonEvent::Approval(ApprovalEvent::Requested { request }),
                ],
                occurred_at_ms: 1,
            })
            .expect("waiting approval should persist");
    }

    let approvals = app
        .list_approvals(
            &session.id,
            &crate::ListApprovalsQuery {
                run_id: Some(run_id.clone()),
                approval_id: None,
            },
        )
        .expect("listing approvals should expire stale request");
    let run = execution
        .load_run_projection(&run_id)
        .expect("run should load after expiry");
    let events = execution
        .store
        .lock()
        .expect("store should not poison")
        .events_for_session(&session.id)
        .expect("events should load");

    assert!(approvals.items.is_empty());
    assert_eq!(run.status, RunStatus::Failed);
    assert!(events.iter().any(|record| {
        matches!(
            &record.payload,
            DaemonEvent::Approval(ApprovalEvent::Resolved { resolution })
                if resolution.approval_id == approval_id
                    && resolution.reason == ApprovalResolutionReason::Expired
        )
    }));
}

#[test]
fn decide_approval_resolves_live_provider_handle_without_restarting_run() {
    let runtime = crate::RuntimeService::bootstrap();
    let (app, execution) = app_and_execution_with_runtime(runtime);
    let session = app
        .open_session(
            TEST_CLIENT_NAME,
            TEST_OWNER_PRINCIPAL_ID,
            &crate::orchestration::OpenSessionRequest {
                title: "Build live approval bridge".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open");
    let running = ensure_running_run(&app, &execution, &session.id, "Resolve live tool approval");
    let resolved = attach_recording_handle(&execution, &running.id);
    let tool_call_id = AgentStreamItemId::new("tool-call-live").expect("tool call id");
    let requested_at_ms = current_time_ms();
    let ttl = ta_policy::ApprovalTtlPolicy::default();
    let approval = ApprovalRequest::new(
        ApprovalId::new("approval-live-tool").expect("approval id"),
        running.id.clone(),
        ApprovalScope::ProcessExec,
        requested_at_ms,
        ttl.expires_at_ms(requested_at_ms),
        ApprovalTarget::ToolCall {
            tool_name: "shell".to_string(),
        },
        "tool shell requires approval",
    )
    .expect("approval request")
    .with_tool_call_id(tool_call_id.clone());
    provider_sink(&execution, &session.id, &running.id)
        .request_approval(approval.clone())
        .expect("approval request should persist");

    let decided = execution
        .decide_approval(
            session.id.clone(),
            approval_actor(),
            DaemonApprovalDecideParams {
                approval_id: approval.id.clone(),
                decision: ApprovalDecision::Approved,
                commentary: Some("allow once".to_string()),
            },
        )
        .expect("live approval should decide");

    assert_eq!(decided.run.status, RunStatus::Running);
    assert!(decided.events.iter().any(|record| {
        matches!(
            &record.payload,
            DaemonEvent::Approval(ApprovalEvent::Resolved { resolution })
                if resolution.approval_id == approval.id
                    && resolution.tool_call_id.as_ref() == Some(&tool_call_id)
        )
    }));
    let recorded = resolved
        .lock()
        .expect("recorded approvals should not be poisoned");
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].approval_id, approval.id);
    assert_eq!(recorded[0].decision, ApprovalDecision::Approved);
}
