mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use support::*;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use taugentic_agent::tools::Registry;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn sequential_fallback() {
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));
    let mut registry = Registry::new();
    let _ = registry.add(CountingTool::new(active, max_active.clone()));
    let client = MockClient::new(
        vec![
            MockStart::Stream(tool_turn("count", 12)),
            MockStart::Stream(end_turn()),
        ],
        false,
    );
    let sink = TestSink::new();
    let session = Session::new(&request());
    let mut loop_state = run_loop(
        client,
        registry,
        MessageQueue::default(),
        session,
        CancellationToken::new(),
        sink,
    );

    let result = loop_state.execute().await;

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(max_active.load(Ordering::SeqCst), 1);
}
