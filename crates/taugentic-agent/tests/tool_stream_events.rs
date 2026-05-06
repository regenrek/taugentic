mod support;

use support::*;
use ta_protocol::wire::{AgentStreamFrame, AgentToolCallOutcome};
use taugentic_agent::queues::MessageQueue;
use taugentic_agent::session::Session;
use taugentic_agent::tools::Registry;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn tool_stream_events_cover_success_and_failure() {
    let mut registry = Registry::new();
    let _ = registry.add(EchoTool);
    let _ = registry.add(FailingTool);
    let client = MockClient::new(
        vec![
            MockStart::Stream(tool_turn_sequence(vec![
                ("echo".to_string(), r#"{"ok":true}"#.to_string()),
                ("fail".to_string(), r#"{}"#.to_string()),
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
    let frames = sink.stream_frames();
    let starts = frames
        .iter()
        .filter(|emission| matches!(emission.frame, AgentStreamFrame::ToolCallStarted { .. }))
        .count();
    assert_eq!(starts, 2);
    assert!(frames.iter().any(|emission| matches!(
        emission.frame,
        AgentStreamFrame::ToolCallCompleted {
            outcome: AgentToolCallOutcome::Completed
        }
    )));
    assert!(frames.iter().any(|emission| matches!(
        emission.frame,
        AgentStreamFrame::ToolCallCompleted {
            outcome: AgentToolCallOutcome::Failed
        }
    )));
}
