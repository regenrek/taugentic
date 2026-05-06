mod support;

use support::*;
use ta_provider_llm::error::LlmClientError;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn stream_model_compaction() {
    let client = MockClient::new(
        vec![
            MockStart::Error(LlmClientError::ContextLengthExceeded(
                "context window exceeded".to_string(),
            )),
            MockStart::Stream(end_turn()),
        ],
        false,
    );
    let sink = TestSink::new();
    let session = Session::new(&request());
    let mut loop_state = run_loop(
        client,
        registry_with_echo(),
        MessageQueue::default(),
        session.clone(),
        CancellationToken::new(),
        sink,
    );

    let result = loop_state.execute().await;

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(session.compact_count().expect("compaction count"), 1);
}
