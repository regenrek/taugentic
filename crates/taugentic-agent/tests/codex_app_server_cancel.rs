mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use ta_provider_llm::families::codex_app_server::CodexAppServerClient;
use taugentic_agent::ExecutionError;
use taugentic_agent::execution_strategy::codex_app_server::dispatch_with_client;

#[test]
fn sandbox_safe_temp_dir_cleans_up_under_tmp() {
    let temp_dir = support::sandbox_safe_temp_dir("sandbox-safe-temp-dir");
    let path = temp_dir.path().to_path_buf();
    assert!(path.is_absolute());
    assert!(path.starts_with("/tmp"));

    fs::write(path.join("marker"), "ok").expect("marker");
    assert!(path.exists());
    drop(temp_dir);
    assert!(!path.exists());
}

#[test]
#[cfg(unix)]
fn codex_app_server_cancel_interrupts_active_turn() {
    let marker_dir = support::sandbox_safe_temp_dir("codex-app-server-cancel-marker");
    let marker = marker_dir.path().join("marker");
    let binary_dir = mock_codex_binary(&format!(
        r#"#!/usr/bin/env python3
import json, select, sys, time
marker = {marker:?}
thread = "thread-1"
turn = "turn-1"
for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialized":
        continue
    if method == "initialize":
        print(json.dumps({{"id": msg["id"], "result": {{}}}}), flush=True)
    elif method == "thread/start":
        print(json.dumps({{"id": msg["id"], "result": {{"thread": {{"id": thread}}}}}}), flush=True)
    elif method == "turn/start":
        print(json.dumps({{"id": msg["id"], "result": {{"turn": {{"id": turn}}}}}}), flush=True)
        for idx in range(20):
            ready, _, _ = select.select([sys.stdin], [], [], 0.05)
            if ready:
                interrupt = json.loads(sys.stdin.readline())
                if interrupt.get("method") == "turn/interrupt":
                    open(marker, "w").write("interrupted")
                    print(json.dumps({{"id": interrupt["id"], "result": {{}}}}), flush=True)
                    sys.exit(0)
            print(json.dumps({{"method":"item/agentMessage/delta","params":{{"threadId":thread,"turnId":turn,"itemId":"item-1","delta":str(idx)}}}}), flush=True)
            time.sleep(0.05)
"#,
        marker = marker.display().to_string()
    ));
    let mut request = support::request();
    support::configure_codex_app_server_request(&mut request);
    let sink = support::TestSink::new();
    let handle = dispatch_with_client(
        request,
        sink.clone(),
        CodexAppServerClient::with_binary(binary_dir.path().join("codex")),
    )
    .expect("handle");

    wait_for(|| !sink.stream_frames().is_empty());
    handle.cancel().expect("cancel");
    wait_for(|| {
        sink.failed
            .lock()
            .expect("failed")
            .iter()
            .any(|error| matches!(error, ExecutionError::Cancelled(_)))
    });
    assert_eq!(fs::read_to_string(marker).expect("marker"), "interrupted");
}

fn mock_codex_binary(body: &str) -> tempfile::TempDir {
    let dir = support::sandbox_safe_temp_dir("codex-app-server-cancel");
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
