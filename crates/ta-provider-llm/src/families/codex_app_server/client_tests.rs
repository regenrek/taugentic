use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

fn unique_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    PathBuf::from("/tmp")
        .join("ta-provider-llm-test-artifacts")
        .join(format!("{name}-{suffix}"))
}

#[cfg(unix)]
fn write_script(name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let dir = unique_dir(name);
    fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("codex");
    fs::write(&path, body).expect("script");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("permissions");
    path
}

#[test]
#[cfg(unix)]
fn run_passes_stdin_to_codex_command() {
    let binary = write_script(
        "stdin",
        "#!/bin/sh\nread value\nprintf 'stdin:%s\\n' \"$value\"\n",
    );
    let cli = CodexCli::with_binary(binary);

    let output = cli
        .run(&["login", "--with-api-key"], Some("sk-test"))
        .expect("output");

    assert_eq!(output.stdout, "stdin:sk-test");
}

#[test]
#[cfg(unix)]
fn run_surfaces_non_zero_exit_as_command_failure() {
    let binary = write_script("failure", "#!/bin/sh\necho 'boom' 1>&2\nexit 7\n");
    let cli = CodexCli::with_binary(binary);

    let error = cli
        .run(&["login", "status"], None)
        .expect_err("should fail");

    assert!(matches!(error, CodexLlmClientError::CommandFailed(message) if message == "boom"));
}

#[test]
#[cfg(unix)]
fn run_with_timeout_kills_stuck_status_probe() {
    let binary = write_script("timeout", "#!/bin/sh\nsleep 5\n");
    let cli = CodexCli::with_binary(binary);

    let error = cli
        .run_with_timeout(&["login", "status"], None, Some(Duration::from_millis(50)))
        .expect_err("should time out");

    assert!(
        matches!(error, CodexLlmClientError::CommandTimedOut(message) if message.contains("login status"))
    );
}

#[test]
#[cfg(unix)]
fn run_with_timeout_returns_fast_successful_status_probe() {
    let binary = write_script(
        "fast-success",
        "#!/bin/sh\nprintf 'Logged in using ChatGPT\\n'\nexit 0\n",
    );
    let cli = CodexCli::with_binary(binary);

    let output = cli
        .run_with_timeout(&["login", "status"], None, Some(Duration::from_millis(200)))
        .expect("fast successful command should not be degraded");

    assert_eq!(output.stdout, "Logged in using ChatGPT");
    assert_eq!(output.stderr, "");
}

#[test]
fn json_rpc_error_response_maps_to_typed_error() {
    let error = parse_json_rpc_error(&json!({
        "code": -32001,
        "message": "Server overloaded; retry later.",
        "data": {"kind": "overloaded"}
    }));
    assert!(matches!(error, CodexLlmClientError::RateLimited { .. }));
}

#[test]
fn json_rpc_id_correlation_rejects_unexpected_response() {
    let binary = write_script(
        "bad-id",
        r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    if msg.get("method") == "initialize":
        print(json.dumps({"id": 999, "result": {}}), flush=True)
        sys.exit(0)
"#,
    );
    let client = CodexAppServerClient::with_binary(binary);
    let result = client.start_control_session();
    let Err(error) = result else {
        panic!("unexpected response id should fail");
    };
    assert!(
        matches!(error, CodexLlmClientError::Protocol(_)),
        "unexpected error: {error:?}"
    );
}
