mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use ta_protocol::wire::AgentStreamFrame;
use ta_provider_llm::families::codex_app_server::CodexAppServerClient;
use taugentic_agent::execution_strategy::codex_app_server::dispatch_with_client;

#[test]
fn codex_app_server_strategy_streams_app_server_events() {
    let binary_dir = mock_codex_binary();
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
    assert_eq!(
        frames,
        vec![
            AgentStreamFrame::AssistantMessageDelta {
                delta: "hello".to_string()
            },
            AgentStreamFrame::AssistantTurnCompleted,
        ]
    );
}

fn mock_codex_binary() -> tempfile::TempDir {
    let dir = support::sandbox_safe_temp_dir("codex-app-server-mock");
    let script = dir.path().join("codex");
    fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialized":
        continue
    if method == "initialize":
        print(json.dumps({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":"1","capabilities":{}}}), flush=True)
    elif method == "thread/start":
        assert "approvalPolicy" not in msg["params"], "turn policy must be sent on turn/start"
        print(json.dumps({"jsonrpc":"2.0","id":msg["id"],"result":{"thread":{"id":"thread-1"}}}), flush=True)
    elif method == "turn/start":
        assert msg["params"]["approvalPolicy"] == "never"
        assert msg["params"]["sandboxPolicy"] == {"type":"dangerFullAccess"}
        print(json.dumps({"jsonrpc":"2.0","id":msg["id"],"result":{"turn":{"id":"turn-1"}}}), flush=True)
        print(json.dumps({"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"thread-1","turnId":"turn-1","itemId":"item-1","delta":"hello"}}), flush=True)
        print(json.dumps({"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread-1","turn":{"id":"turn-1","status":"completed"}}}), flush=True)
"#,
    )
    .expect("script");
    let mut permissions = fs::metadata(&script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).expect("permissions");
    dir
}

fn wait_for(condition: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(condition());
}
