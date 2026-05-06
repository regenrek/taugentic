mod support;

use support::*;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn steering_queue() {
    let client = MockClient::new(
        vec![
            MockStart::Stream(tool_turn("echo", 1)),
            MockStart::Stream(end_turn()),
        ],
        true,
    );
    let mut queues = MessageQueue::default();
    queues.push_steering(user_message("steer between turns"));
    let sink = TestSink::new();
    let session = Session::new(&request());
    let mut loop_state = run_loop(
        client.clone(),
        registry_with_echo(),
        queues,
        session,
        CancellationToken::new(),
        sink,
    );

    let result = loop_state.execute().await;

    assert!(result.is_ok(), "{result:?}");
    let requests = client.requests();
    assert_eq!(requests.len(), 2);
    let second = &requests[1].messages;
    let tool_pos = second
        .iter()
        .position(|message| message.tool_call_id.as_deref() == Some("tool-call-0"));
    let steer_pos = second
        .iter()
        .position(|message| message.content == "steer between turns");
    assert!(tool_pos.is_some());
    assert!(steer_pos.is_some());
    assert!(tool_pos < steer_pos);
}
