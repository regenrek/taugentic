mod support;

use support::*;
use ta_protocol::wire::AgentStreamFrame;
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use taugentic_agent::tools::Registry;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn stream_correlation() {
    let mut registry = Registry::new();
    let _ = registry.add(DelayTool);
    let client = MockClient::new(
        vec![
            MockStart::Stream(tool_turn_sequence(vec![
                ("delay".to_string(), r#"{"delay_ms":80}"#.to_string()),
                ("delay".to_string(), r#"{"delay_ms":1}"#.to_string()),
                ("delay".to_string(), r#"{"delay_ms":20}"#.to_string()),
            ])),
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
        sink.clone(),
    );

    let result = loop_state.execute().await;

    assert!(result.is_ok(), "{result:?}");
    let completed = sink
        .stream_frames()
        .into_iter()
        .filter_map(|emission| match emission.frame {
            AgentStreamFrame::ToolCallCompleted { .. } => {
                emission.item_id.map(|item_id| item_id.as_str().to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        completed,
        vec!["tool-call-0", "tool-call-1", "tool-call-2"],
        "parallel completion races must still emit deterministic call order"
    );
}
