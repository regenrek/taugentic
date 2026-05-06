mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use support::*;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use taugentic_agent::tools::Registry;
use taugentic_agent::turn_loop::MAX_CONCURRENT_TOOLS;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn parallel_tool_execution() {
    let active = Arc::new(AtomicUsize::new(0));
    let max_active = Arc::new(AtomicUsize::new(0));
    let mut registry = Registry::new();
    let _ = registry.add(CountingTool::new(active, max_active.clone()));
    let client = MockClient::new(
        vec![
            MockStart::Stream(tool_turn("count", 12)),
            MockStart::Stream(end_turn()),
        ],
        true,
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
    let max = max_active.load(Ordering::SeqCst);
    assert!(max > 1, "expected parallel execution, saw {max}");
    assert!(max <= MAX_CONCURRENT_TOOLS, "max active {max}");
}
