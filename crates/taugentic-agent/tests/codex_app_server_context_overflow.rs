mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use ta_provider_llm::families::codex_app_server::CodexAppServerClient;
use taugentic_agent::ExecutionError;
use taugentic_agent::execution_strategy::codex_app_server::dispatch_with_client;

#[test]
#[cfg(unix)]
fn codex_app_server_context_overflow_maps_to_execution_error() {
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
        emit({"method":"error","params":{"threadId":thread,"turnId":turn,"willRetry":False,"error":{"message":"context window exceeded","codexErrorInfo":"contextWindowExceeded","additionalDetails":None}}})
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

    wait_for(|| !sink.failed.lock().expect("failed").is_empty());
    drop(handle);

    let failures = sink.failed.lock().expect("failed").clone();
    assert!(
        failures
            .iter()
            .any(|error| matches!(error, ExecutionError::ContextLengthExceeded(detail) if detail.contains("context window"))),
        "expected context length mapping, got {failures:?}"
    );
}

fn mock_codex_binary(body: &str) -> tempfile::TempDir {
    let dir = support::sandbox_safe_temp_dir("codex-app-server-context");
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
