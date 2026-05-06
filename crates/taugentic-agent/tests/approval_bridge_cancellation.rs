mod support;

use support::*;
use ta_protocol::wire::ApprovalScope;
use taugentic_agent::ExecutionError;
use taugentic_agent::approval::{ApprovalBridge, ApprovalDescriptor};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn approval_bridge_cancellation_wait_has_cancel_escape() {
    let sink = TestSink::new();
    let request = request();
    let cancellation = CancellationToken::new();
    let bridge = ApprovalBridge::new(request.run_id.clone(), sink, cancellation.clone());
    let descriptor =
        ApprovalDescriptor::new("tool-call-cancel", "shell", "tool shell requires approval");
    let id = bridge
        .request(ApprovalScope::ProcessExec, &descriptor)
        .expect("approval request");

    cancellation.cancel();
    let result = bridge.wait(id).await;

    assert!(matches!(result, Err(ExecutionError::Cancelled(_))));
}
