mod support;

use support::*;
use ta_provider_llm::error::LlmClientError;
use taugentic_agent::ExecutionError;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use taugentic_agent::turn_loop::MAX_CONTEXT_LIMIT_RETRIES;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn context_limit_retry_cap() {
    let starts = (0..=MAX_CONTEXT_LIMIT_RETRIES)
        .map(|_| {
            MockStart::Error(LlmClientError::ContextLengthExceeded(
                "too much context".to_string(),
            ))
        })
        .collect();
    let client = MockClient::new(starts, true);
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

    assert!(matches!(
        result,
        Err(ExecutionError::ContextLengthExceeded(_))
    ));
    assert_eq!(client.requests().len(), MAX_CONTEXT_LIMIT_RETRIES + 1);
    assert_eq!(
        session.compact_count().unwrap_or_default(),
        MAX_CONTEXT_LIMIT_RETRIES
    );
}
