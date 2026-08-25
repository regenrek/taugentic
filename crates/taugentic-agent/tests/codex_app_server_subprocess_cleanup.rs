mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use ta_provider_llm::families::codex_app_server::CodexAppServerClient;
use taugentic_agent::ExecutionError;
use taugentic_agent::execution_strategy::codex_app_server::dispatch_with_client;

#[test]
#[cfg(unix)]
fn dropping_codex_app_server_handle_kills_subprocess() {
    let pid_dir = support::sandbox_safe_temp_dir("codex-app-server-pid");
    let pid_file = pid_dir.path().join("pid");
    let binary_dir = mock_codex_binary(&format!(
        r#"#!/usr/bin/env python3
import json, os, sys, time
open({pid_file:?}, "w").write(str(os.getpid()))
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
        while True:
            time.sleep(1)
"#,
        pid_file = pid_file.display().to_string()
    ));
    let mut request = support::request();
    support::configure_codex_app_server_request(&mut request);
    let sink = support::TestSink::new();
    let handle = dispatch_with_client(
        request,
        sink,
        CodexAppServerClient::with_binary(binary_dir.path().join("codex")),
    )
    .expect("handle");

    wait_for(|| pid_file.exists());
    let pid = fs::read_to_string(&pid_file)
        .expect("pid")
        .trim()
        .to_string();
    assert!(process_exists(&pid));
    drop(handle);
    wait_for(|| !process_exists(&pid));
}

#[test]
#[cfg(unix)]
fn idle_codex_turn_fails_and_terminates_instead_of_running_forever() {
    let pid_dir = support::sandbox_safe_temp_dir("codex-app-server-idle-pid");
    let pid_file = pid_dir.path().join("pid");
    let binary_dir = mock_codex_binary(&format!(
        r#"#!/usr/bin/env python3
import json, os, sys, time
open({pid_file:?}, "w").write(str(os.getpid()))
for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialized":
        continue
    if method == "initialize":
        print(json.dumps({{"id": msg["id"], "result": {{}}}}), flush=True)
    elif method == "thread/start":
        print(json.dumps({{"id": msg["id"], "result": {{"thread": {{"id": "thread-1"}}}}}}), flush=True)
    elif method == "turn/start":
        print(json.dumps({{"id": msg["id"], "result": {{"turn": {{"id": "turn-1"}}}}}}), flush=True)
        while True:
            time.sleep(1)
"#,
        pid_file = pid_file.display().to_string()
    ));
    let mut request = support::request();
    support::configure_codex_app_server_request(&mut request);
    let sink = support::TestSink::new();
    let handle = dispatch_with_client(
        request,
        sink.clone(),
        CodexAppServerClient::with_binary_and_turn_idle_timeout(
            binary_dir.path().join("codex"),
            Duration::from_millis(100),
        ),
    )
    .expect("handle");

    wait_for(|| {
        sink.failed
            .lock()
            .expect("failed")
            .iter()
            .any(|error| matches!(error, ExecutionError::ProcessTimeout { .. }))
    });
    let pid = fs::read_to_string(&pid_file)
        .expect("pid")
        .trim()
        .to_string();
    wait_for(|| !process_exists(&pid));
    drop(handle);
}

fn process_exists(pid: &str) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn mock_codex_binary(body: &str) -> tempfile::TempDir {
    let dir = support::sandbox_safe_temp_dir("codex-app-server-cleanup");
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
