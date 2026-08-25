mod support;

use support::*;
use ta_protocol::wire::{AgentStreamFrame, RuntimeLanePendingState};
use taugentic_agent::approval::ApprovalOutcome;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::tools::Registry;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn stream_correlation_approvals_uses_triggering_tool_call_id() {
    let mut registry = Registry::new();
    let _ = registry.add(ApprovalTool);
    let client = MockClient::new(
        vec![MockStart::Stream(tool_turn("approval_tool", 1))],
        false,
    );
    let sink = TestSink::new();
    let request = request_requiring_approval();
    let cancellation = CancellationToken::new();
    let context = LoopApprovalContext::new(request, sink.clone(), cancellation);
    let bridge = context.approval_bridge.clone();
    let mut loop_state = run_loop_with_bridge(client, registry, MessageQueue::default(), context);

    let task = tokio::spawn(async move { loop_state.execute().await });
    let request = resolve_first_approval(&sink, &bridge, ApprovalOutcome::Allow).await;
    let result = task.await.expect("turn task should join");

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        request
            .tool_call_id
            .as_ref()
            .map(ta_protocol::wire::AgentStreamItemId::as_str),
        Some("tool-call-0")
    );
    assert_eq!(
        sink.approval_resolutions()[0].approval_id.as_str(),
        request.id.as_str()
    );
    assert_eq!(
        sink.approval_resolutions()[0]
            .tool_call_id
            .as_ref()
            .map(ta_protocol::wire::AgentStreamItemId::as_str),
        Some("tool-call-0")
    );
    assert!(sink.stream_frames().iter().any(|emission| {
        matches!(
            emission.frame,
            AgentStreamFrame::PendingStateChanged {
                state: RuntimeLanePendingState::WaitingForApproval
            }
        ) && emission
            .item_id
            .as_ref()
            .is_some_and(|item_id| item_id.as_str() == "tool-call-0")
    }));
}
