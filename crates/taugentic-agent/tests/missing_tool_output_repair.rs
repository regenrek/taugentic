mod support;

use support::*;
use ta_provider_llm::client::StreamToolCallRecord;
use taugentic_agent::session::{Session, assistant_tool_message};

#[test]
fn missing_tool_output_repair() {
    let session = Session::from_history(
        vec![
            user_message("start"),
            assistant_tool_message(vec![StreamToolCallRecord {
                id: "call-missing".to_string(),
                name: "echo".to_string(),
                input: serde_json::json!({"n":1}),
            }]),
        ],
        None,
    );

    let repaired = session.repair_missing_tool_outputs();

    assert_eq!(repaired.unwrap_or_default(), 1);
    assert!(session.history().unwrap_or_default().iter().any(|message| {
        message.tool_call_id.as_deref() == Some("call-missing")
            && message.content.contains("missing_tool_output_repaired")
    }));
}
