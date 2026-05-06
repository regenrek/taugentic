mod support;

use support::*;
use ta_provider_llm::error::LlmClientError;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn compaction_on_stream_open() {
    let client = MockClient::new(
        vec![
            MockStart::Error(LlmClientError::ContextLengthExceeded(
                "open failed".to_string(),
            )),
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
        sink,
    );

    let result = loop_state.execute().await;

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(client.requests().len(), 2);
    assert_eq!(session.compact_count().unwrap_or_default(), 1);
}
