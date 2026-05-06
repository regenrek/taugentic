mod support;

use support::*;
use taugentic_agent::ExecutionError;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::ApprovalStatus;
use taugentic_agent::tools::Registry;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn approval_bridge_turn_interrupt_rejects_pending() {
    let mut registry = Registry::new();
    let _ = registry.add(ApprovalTool);
    let client = MockClient::new(
        vec![MockStart::Stream(tool_turn("approval_tool", 1))],
        false,
    );
    let sink = TestSink::new();
    let request = request();
    let cancellation = CancellationToken::new();
    let context = LoopApprovalContext::new(request, sink.clone(), cancellation);
    let session = context.session.clone();
    let bridge = context.approval_bridge.clone();
    let mut loop_state = run_loop_with_bridge(client, registry, MessageQueue::default(), context);

    let task = tokio::spawn(async move { loop_state.execute().await });
    let request = wait_for_approval_request(&sink).await;
    bridge.reject_all("turn_interrupted");
    let result = task.await.expect("turn task should join");

    assert!(matches!(result, Err(ExecutionError::Cancelled(_))));
    let pending = session.pending_approvals().expect("pending approvals");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, request.id.as_str());
    assert!(matches!(
        &pending[0].status,
        ApprovalStatus::Rejected { reason } if reason == "turn_interrupted"
    ));
}
