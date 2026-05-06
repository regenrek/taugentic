mod support;

use support::*;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn turn_loop_basic() {
    let client = MockClient::new(
        vec![
            MockStart::Stream(tool_turn("echo", 1)),
            MockStart::Stream(end_turn()),
        ],
        true,
    );
    let sink = TestSink::new();
    let session = Session::new(&request());
    let mut loop_state = run_loop(
        client.clone(),
        registry_with_echo(),
        MessageQueue::default(),
        session.clone(),
        CancellationToken::new(),
        sink.clone(),
    );

    let result = loop_state.execute().await;

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(client.requests().len(), 2);
    assert!(
        session
            .history()
            .unwrap_or_default()
            .iter()
            .any(|message| message.tool_call_id.as_deref() == Some("tool-call-0"))
    );
    assert_eq!(
        sink.completed.lock().map(|items| items.len()).unwrap_or(0),
        1
    );
}
