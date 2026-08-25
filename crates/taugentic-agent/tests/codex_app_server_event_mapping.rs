mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use ta_protocol::wire::{AgentStreamFrame, AgentToolCallOutcome};
use ta_provider_llm::families::codex_app_server::CodexAppServerClient;
use taugentic_agent::execution_strategy::codex_app_server::dispatch_with_client;

#[test]
#[cfg(unix)]
fn codex_app_server_maps_supported_event_types() {
    let binary_dir = mock_codex_binary(
        r#"#!/usr/bin/env python3
import json, sys
thread = "thread-1"
turn = "turn-1"
def emit(value):
    print(json.dumps(value), flush=True)
for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialized":
        continue
    if method == "initialize":
        emit({"id": msg["id"], "result": {}})
    elif method == "thread/start":
        emit({"id": msg["id"], "result": {"thread": {"id": thread}}})
    elif method == "turn/start":
        emit({"id": msg["id"], "result": {"turn": {"id": turn}}})
        emit({"method":"turn/started","params":{"threadId":thread,"turn":{"id":turn,"items":[],"status":"running","error":None,"startedAt":None,"completedAt":None,"durationMs":None}}})
        emit({"method":"item/started","params":{"threadId":thread,"turnId":turn,"item":{"type":"commandExecution","id":"cmd-1","command":"echo hi","cwd":"/tmp","processId":None,"source":"agent","status":"inProgress","commandActions":[],"aggregatedOutput":None,"exitCode":None,"durationMs":None}}})
        emit({"method":"item/commandExecution/outputDelta","params":{"threadId":thread,"turnId":turn,"itemId":"cmd-1","delta":"stdout"}})
        emit({"method":"item/completed","params":{"threadId":thread,"turnId":turn,"item":{"type":"commandExecution","id":"cmd-1","command":"echo hi","cwd":"/tmp","processId":None,"source":"agent","status":"completed","commandActions":[],"aggregatedOutput":"stdout","exitCode":0,"durationMs":1}}})
        emit({"method":"item/started","params":{"threadId":thread,"turnId":turn,"item":{"type":"mcpToolCall","id":"mcp-1","server":"srv","tool":"lookup","status":"inProgress","arguments":{},"result":None,"error":None,"durationMs":None}}})
        emit({"method":"item/mcpToolCall/progress","params":{"threadId":thread,"turnId":turn,"itemId":"mcp-1","message":"working"}})
        emit({"method":"item/completed","params":{"threadId":thread,"turnId":turn,"item":{"type":"mcpToolCall","id":"mcp-1","server":"srv","tool":"lookup","status":"failed","arguments":{},"result":None,"error":{"message":"nope"},"durationMs":1}}})
        emit({"method":"item/reasoning/textDelta","params":{"threadId":thread,"turnId":turn,"itemId":"reason-1","delta":"thinking","contentIndex":0}})
        emit({"method":"thread/tokenUsage/updated","params":{"threadId":thread,"turnId":turn,"tokenUsage":{"total":{"totalTokens":42,"inputTokens":10,"cachedInputTokens":0,"outputTokens":32,"reasoningOutputTokens":4},"last":{"totalTokens":42,"inputTokens":10,"cachedInputTokens":0,"outputTokens":32,"reasoningOutputTokens":4},"modelContextWindow":128000}}})
        emit({"method":"item/autoApprovalReview/started","params":{"threadId":thread,"turnId":turn,"reviewId":"review-1","targetItemId":"cmd-1","review":{"status":"inProgress","userAuthorization":None},"action":{"type":"command","command":"echo hi","cwd":"/tmp"}}})
        emit({"method":"item/agentMessage/delta","params":{"threadId":thread,"turnId":turn,"itemId":"msg-1","delta":"done"}})
        emit({"method":"item/completed","params":{"threadId":thread,"turnId":turn,"item":{"type":"agentMessage","id":"msg-1","text":"done"}}})
        emit({"method":"turn/completed","params":{"threadId":thread,"turn":{"id":turn,"items":[],"status":"completed","error":None,"startedAt":None,"completedAt":None,"durationMs":None}}})
"#,
    );
    let mut request = support::request();
    support::configure_codex_app_server_request(&mut request);
    let sink = support::TestSink::new();
    let handle = dispatch_with_client(
        request,
        sink.clone(),
        CodexAppServerClient::with_binary(binary_dir.path().join("codex")),
    )
    .expect("handle");

    wait_for(|| !sink.completed.lock().expect("complete").is_empty());
    drop(handle);

    let frames = sink
        .stream_frames()
        .into_iter()
        .map(|emission| emission.frame)
        .collect::<Vec<_>>();
    assert!(frames.contains(&AgentStreamFrame::AssistantTurnStarted));
    assert!(frames.contains(&AgentStreamFrame::ToolCallStarted {
        tool_name: "codex/command_execution".to_string(),
        input: "null".to_string()
    }));
    assert!(frames.contains(&AgentStreamFrame::ToolCallProgressed {
        delta: "stdout".to_string()
    }));
    assert!(frames.contains(&AgentStreamFrame::ToolCallCompleted {
        outcome: AgentToolCallOutcome::Completed
    }));
    assert!(frames.contains(&AgentStreamFrame::ToolCallStarted {
        tool_name: "codex/mcp/srv/lookup".to_string(),
        input: "null".to_string()
    }));
    assert!(frames.contains(&AgentStreamFrame::ToolCallCompleted {
        outcome: AgentToolCallOutcome::Failed
    }));
    assert_eq!(
        frames
            .iter()
            .filter(|frame| {
                **frame
                    == AgentStreamFrame::AssistantMessageDelta {
                        delta: "done".to_string(),
                    }
            })
            .count(),
        1
    );
    assert!(frames.contains(&AgentStreamFrame::AssistantTurnCompleted));
    assert!(frames.contains(&AgentStreamFrame::TokenUsageUpdated {
        total_tokens: Some(42),
        model_context_window: Some(128000)
    }));
    let activities = sink.activities();
    assert!(
        activities
            .iter()
            .any(|activity| activity.contains("codex approval requested"))
    );
}

fn mock_codex_binary(body: &str) -> tempfile::TempDir {
    let dir = support::sandbox_safe_temp_dir("codex-app-server-events");
    let script = dir.path().join("codex");
    fs::write(&script, body).expect("script");
    let mut permissions = fs::metadata(&script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).expect("permissions");
    dir
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(condition());
}
