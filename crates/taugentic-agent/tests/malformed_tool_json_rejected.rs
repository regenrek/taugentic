mod support;

use support::*;
use ta_provider_llm::client::StreamEvent;
use taugentic_agent::ExecutionError;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn malformed_tool_json_rejected() {
    let client = MockClient::new(
        vec![MockStart::Stream(vec![
            Ok(StreamEvent::ToolCallStarted {
                id: "bad-json".to_string(),
                index: 0,
                name: "echo".to_string(),
            }),
            Ok(StreamEvent::ToolInputDelta {
                id: "bad-json".to_string(),
                index: 0,
                delta: r#"{"unterminated":true"#.to_string(),
            }),
            Ok(StreamEvent::ToolCallBatchCompleted),
        ])],
        false,
    );
    let sink = TestSink::new();
    let session = Session::new(&request());
    let mut loop_state = run_loop(
        client,
        registry_with_echo(),
        MessageQueue::default(),
        session,
        CancellationToken::new(),
        sink,
    );

    let result = loop_state.execute().await;

    assert!(matches!(result, Err(ExecutionError::InvalidToolInput(_))));
}
