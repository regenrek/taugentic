mod support;

use support::*;
use ta_protocol::wire::{ApprovalDecision, ApprovalScope};
use taugentic_agent::approval::{ApprovalBridge, ApprovalDescriptor, ApprovalOutcome};
use tokio::time::{Duration, timeout};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn approval_bridge_idempotent_resolve_first_resolution_wins() {
    let sink = TestSink::new();
    let request = request();
    let cancellation = CancellationToken::new();
    let bridge = ApprovalBridge::new(request.run_id.clone(), sink.clone(), cancellation);
    let descriptor = ApprovalDescriptor::new(
        "tool-call-idempotent",
        "shell",
        "tool shell requires approval",
    );
    let id = bridge
        .request(ApprovalScope::ProcessExec, &descriptor)
        .expect("approval request");

    bridge.resolve(id.clone(), ApprovalOutcome::Allow);
    bridge.resolve(id.clone(), ApprovalOutcome::Allow);
    bridge.resolve(id.clone(), ApprovalOutcome::Deny);
    let outcome = bridge.wait(id).await.expect("approval wait");

    assert_eq!(outcome, ApprovalOutcome::Allow);
    let resolutions = sink.approval_resolutions();
    assert_eq!(resolutions.len(), 1);
    assert_eq!(resolutions[0].decision, ApprovalDecision::Approved);
}

#[tokio::test]
async fn approval_bridge_does_not_wake_waiter_when_resolution_event_fails() {
    let sink = TestSink::new();
    let request = request();
    let cancellation = CancellationToken::new();
    let bridge = ApprovalBridge::new(request.run_id.clone(), sink.clone(), cancellation);
    let descriptor = ApprovalDescriptor::new(
        "tool-call-durable-resolution",
        "shell",
        "tool shell requires approval",
    );
    let id = bridge
        .request(ApprovalScope::ProcessExec, &descriptor)
        .expect("approval request");

    sink.set_approval_resolution_failure(true);
    bridge.resolve(id.clone(), ApprovalOutcome::Allow);
    let wait_result = timeout(Duration::from_millis(25), bridge.wait(id.clone())).await;

    assert!(
        wait_result.is_err(),
        "waiter woke before durable resolution"
    );
    assert!(sink.approval_resolutions().is_empty());

    sink.set_approval_resolution_failure(false);
    bridge.resolve(id.clone(), ApprovalOutcome::Allow);
    let outcome = bridge.wait(id).await.expect("approval wait");

    assert_eq!(outcome, ApprovalOutcome::Allow);
    assert_eq!(sink.approval_resolutions().len(), 1);
}
