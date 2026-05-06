mod support;

use support::*;
use taugentic_agent::ExecutionError;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::{ApprovalStatus, Session};
use taugentic_agent::tools::Registry;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn approval_interrupt_resolves_pending() {
    let mut registry = Registry::new();
    let _ = registry.add(ApprovalTool);
    let client = MockClient::new(
        vec![MockStart::Stream(tool_turn("approval_tool", 1))],
        false,
    );
    let sink = TestSink::new();
    let session = Session::new(&request());
    let cancellation = CancellationToken::new();
    let mut loop_state = run_loop(
        client,
        registry,
        MessageQueue::default(),
        session.clone(),
        cancellation.clone(),
        sink.clone(),
    );

    let task = tokio::spawn(async move { loop_state.execute().await });
    for _ in 0..20 {
        if !session.pending_approvals().unwrap_or_default().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    cancellation.cancel();
    let result = task
        .await
        .unwrap_or_else(|error| Err(ExecutionError::ProcessFailed(error.to_string())));

    assert!(matches!(result, Err(ExecutionError::Cancelled(_))));
    let pending = session.pending_approvals().unwrap_or_default();
    assert_eq!(pending.len(), 1);
    assert!(matches!(
        &pending[0].status,
        ApprovalStatus::Rejected { reason } if reason == "turn_interrupted"
    ));
    assert!(sink.stream_frames().iter().any(|emission| matches!(
        emission.frame,
        ta_protocol::wire::AgentStreamFrame::PendingStateChanged { .. }
    )));
}
