mod support;

use support::*;
use taugentic_agent::approval::ApprovalOutcome;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::tools::Registry;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn approval_bridge_allow_runs_tool() {
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
    let bridge = context.approval_bridge.clone();
    let mut loop_state = run_loop_with_bridge(client, registry, MessageQueue::default(), context);

    let task = tokio::spawn(async move { loop_state.execute().await });
    resolve_first_approval(&sink, &bridge, ApprovalOutcome::Allow).await;
    let result = task.await.expect("turn task should join");

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        sink.approval_resolutions()[0].decision,
        ta_protocol::wire::ApprovalDecision::Approved
    );
}
