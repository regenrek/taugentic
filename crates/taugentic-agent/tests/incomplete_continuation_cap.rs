mod support;

use support::*;
use taugentic_agent::ExecutionError;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use taugentic_agent::turn_loop::MAX_INCOMPLETE_CONTINUATION_ATTEMPTS;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn incomplete_continuation_cap() {
    let starts = (0..=MAX_INCOMPLETE_CONTINUATION_ATTEMPTS)
        .map(|_| MockStart::Stream(max_tokens_turn()))
        .collect();
    let client = MockClient::new(starts, true);
    let sink = TestSink::new();
    let session = Session::new(&request());
    let mut loop_state = run_loop(
        client.clone(),
        registry_with_echo(),
        MessageQueue::default(),
        session,
        CancellationToken::new(),
        sink,
    );

    let result = loop_state.execute().await;

    assert!(
        matches!(result, Err(ExecutionError::ProcessFailed(detail)) if detail.contains("max continuation attempts reached"))
    );
    assert_eq!(
        client.requests().len(),
        MAX_INCOMPLETE_CONTINUATION_ATTEMPTS + 1
    );
}
