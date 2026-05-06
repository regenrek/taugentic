use ta_provider_llm::client::StopReason;
use taugentic_agent::turn_loop::{ToolCall, filter_truncated_tool_calls};

#[test]
fn truncated_tool_call_filter() {
    let calls = vec![
        ToolCall::new("bad", "echo", r#"{"n":1"#),
        ToolCall::new("good", "echo", r#"{"n":2}"#),
    ];

    let filtered = filter_truncated_tool_calls(StopReason::MaxTokens, calls);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "good");
}
