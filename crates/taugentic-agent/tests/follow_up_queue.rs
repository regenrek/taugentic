mod support;

use support::*;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn follow_up_queue() {
    let client = MockClient::new(
        vec![MockStart::Stream(end_turn()), MockStart::Stream(end_turn())],
        true,
    );
    let mut queues = MessageQueue::default();
    queues.push_follow_up(user_message("follow up after inner loop"));
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
    assert!(
        requests[1]
            .messages
            .iter()
            .any(|message| message.content == "follow up after inner loop")
    );
}
