use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::Command,
    thread,
    thread::JoinHandle,
    time::{SystemTime, UNIX_EPOCH},
};

use assert_cmd::assert::OutputAssertExt;
use assert_cmd::cargo::CommandCargoExt;
use predicates::str::{contains, is_empty};
use serde_json::json;
use sha2::{Digest, Sha256};
use ta_jsonrpc::{
    JsonLineCodec, JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, RequestId, ServerConfig,
    SocketAddress, SocketListener, bind_listener, connect_socket, parse_params,
};
#[cfg(unix)]
use ta_jsonrpc::{JsonRpcError, JsonRpcErrorObject};
use ta_observability::LOG_DIR_ENV_VAR;
use ta_protocol::local_control::RuntimeControlBootstrapCommand;
use ta_protocol::wire::{
    AgentRuntimeModelId, AgentRuntimeSelection, ApprovalDecision, AuthProfileId,
    DAEMON_PROTOCOL_VERSION, DaemonApprovalDecideParams, DaemonApprovalDecideResult,
    DaemonClientCapabilities, DaemonRuntimeMode, DaemonSessionAttachParams,
    DaemonSessionAttachResult, DaemonSessionOpenParams, DaemonSessionOpenResult,
    DaemonStatusParams, DaemonStatusResult, ListSessionsQuery, METHOD_DAEMON_APPROVAL_DECIDE,
    METHOD_DAEMON_CONTROL_STATUS, METHOD_DAEMON_INITIALIZE, METHOD_DAEMON_RUN_START,
    METHOD_DAEMON_SESSION_ATTACH, METHOD_DAEMON_SESSION_LIST, METHOD_DAEMON_SESSION_OPEN,
    METHOD_DAEMON_STATUS, RunId, RunStatus, RunSummary, RuntimeProfileId, SessionAuthority,
    SessionId, SessionNextRunSelection, SessionStatus, SessionSummary, StartRunCommand,
    WorkspaceSelector,
};

#[cfg(unix)]
enum ServerResponse {
    Status(DaemonStatusResult),
    Error(JsonRpcErrorObject),
}

const TEST_SERVER_SHUTDOWN_METHOD: &str = "__test.shutdown__";
const TEST_CLIENT_CREDENTIAL: &str = "credential-1credential-1credential-1";
const TEST_SESSION_AUTHORITY: &str = "session-authority-1session-authority-1";

#[test]
fn help_surfaces_include_expected_commands() {
    let cases: &[(&[&str], &[&str])] = &[
        (
            &["--help"],
            &[
                "Usage: ta",
                "daemon",
                "session",
                "approval",
                "run",
                "workspace",
            ],
        ),
        (
            &["daemon", "--help"],
            &[
                "background",
                "status",
                "start",
                "wait",
                "restart",
                "logs",
                "stop",
            ],
        ),
        (
            &["daemon", "background", "--help"],
            &["status", "enable", "disable"],
        ),
        (&["session", "--help"], &["list", "open"]),
        (&["workspace", "--help"], &["open", "list", "get"]),
        (&["approval", "--help"], &["list", "decide"]),
        (&["run", "--help"], &["list", "start"]),
        (
            &["run", "start", "--help"],
            &["--runtime-profile", "--model", "--auth-profile"],
        ),
    ];

    for (args, expected_fragments) in cases {
        assert_help_surface(args, expected_fragments);
    }
}

/// Exercises the session-list local IPC contract; Windows currently aborts
/// inside the named-pipe contract harness before command logic is reached.
#[cfg(unix)]
#[test]
fn session_list_json_smoke_outputs_parseable_payload() {
    let socket_name = unique_socket_name("ta-cli-session-list");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let server_handle = spawn_session_list_server(
        listener,
        vec![SessionSummary {
            id: SessionId::new("session-1").expect("session id"),
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Idle,
            next_run_selection: SessionNextRunSelection::Unselected,
        }],
    );

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["session", "list", "--json", "--socket", &socket_name])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(
        value,
        json!([{
            "id": "session-1",
            "title": "Build daemon app server",
            "status": "idle",
            "nextRunSelection": { "kind": "unselected" }
        }])
    );
}

/// Exercises the session-open local IPC contract; Windows currently aborts
/// inside the named-pipe contract harness before command logic is reached.
#[cfg(unix)]
#[test]
fn session_open_json_smoke_outputs_parseable_payload() {
    let socket_name = unique_socket_name("ta-cli-session-open");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let workspace_path = "/tmp/taugentic-cli-session-open-missing".to_string();
    let server_handle = spawn_session_open_server(
        listener,
        "Build daemon app server",
        &workspace_path,
        false,
        SessionSummary {
            id: SessionId::new("session-2").expect("session id"),
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Idle,
            next_run_selection: SessionNextRunSelection::Unselected,
        },
    );

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "session",
            "open",
            "Build daemon app server",
            "--workspace",
            &workspace_path,
            "--json",
            "--socket",
            &socket_name,
        ])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["id"], json!("session-2"));
    assert_eq!(value["title"], json!("Build daemon app server"));
    assert_eq!(value["status"], json!("idle"));
}

#[cfg(unix)]
#[test]
fn session_open_trust_workspace_sets_workspace_selector_trust_acknowledged() {
    let socket_name = unique_socket_name("ta-cli-session-open-trust");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let workspace_path = "/tmp/taugentic-cli-session-open-trusted".to_string();
    let server_handle = spawn_session_open_server(
        listener,
        "Trusted daemon app server",
        &workspace_path,
        true,
        SessionSummary {
            id: SessionId::new("session-3").expect("session id"),
            title: "Trusted daemon app server".to_string(),
            status: SessionStatus::Idle,
            next_run_selection: SessionNextRunSelection::Unselected,
        },
    );

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "--trust-workspace",
            &workspace_path,
            "session",
            "open",
            "Trusted daemon app server",
            "--workspace",
            &workspace_path,
            "--json",
            "--socket",
            &socket_name,
        ])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Exercises the run-start persistent local IPC contract; Windows currently
/// aborts inside the named-pipe contract harness before command logic is reached.
#[cfg(unix)]
#[test]
fn run_start_json_smoke_uses_persistent_session_first_flow() {
    let socket_name = unique_socket_name("ta-cli-run-start");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    seed_local_client_credential(&socket_address, "ta-cli");
    seed_local_session_authority(&socket_address, "ta-cli", "session-1");
    let server_handle = spawn_run_start_server(listener);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "run",
            "start",
            "--session",
            "session-1",
            "--runtime-profile",
            "runtime-codex-safe",
            "--model",
            "gpt-5.6-sol",
            "--auth-profile",
            "profile-codex-test",
            "Ship app server hard cut",
            "--json",
            "--socket",
            &socket_name,
        ])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["id"], json!("run-1"));
    assert_eq!(value["objective"], json!("Ship app server hard cut"));
    assert_eq!(value["status"], json!("waitingForApproval"));
}

#[cfg(not(windows))]
#[test]
fn approval_decide_json_smoke_uses_persistent_session_first_flow() {
    let socket_name = unique_socket_name("ta-cli-approval-decide");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    seed_local_client_credential(&socket_address, "ta-cli");
    seed_local_session_authority(&socket_address, "ta-cli", "session-1");
    let server_handle = spawn_approval_decide_server(listener);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "approval",
            "decide",
            "--session",
            "session-1",
            "--approval",
            "approval-1",
            "--decision",
            "approved",
            "--json",
            "--socket",
            &socket_name,
        ])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["id"], json!("run-1"));
    assert_eq!(value["status"], json!("running"));
}

#[test]
fn rejects_unknown_subcommands_with_usage_error() {
    let mut command = Command::cargo_bin("ta-cli").expect("ta-cli binary should build");

    command
        .arg("bogus")
        .assert()
        .failure()
        .code(2)
        .stdout(is_empty())
        .stderr(contains("unrecognized subcommand"))
        .stderr(contains("Usage: ta"));
}

/// Exercises the local IPC daemon-status contract; Windows currently aborts
/// inside the named-pipe contract harness before daemon-status logic is reached.
#[cfg(unix)]
#[test]
fn daemon_status_json_smoke_outputs_parseable_payload() {
    let socket_name = unique_socket_name("ta-cli-daemon-status");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let expected_status = DaemonStatusResult {
        ready: true,
        daemon_instance_id: "daemon-1".to_string(),
        runtime_mode: DaemonRuntimeMode::Local,
        socket_path: "/tmp/ta-cli.sock".to_string(),
        log_path: "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl".to_string(),
        version: "0.0.1-test".to_string(),
    };

    let server_handle =
        spawn_daemon_server(listener, ServerResponse::Status(expected_status), None);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "status", "--json", "--socket", &socket_name])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["ready"], json!(true));
    assert_eq!(value["runtimeMode"], json!("local"));
    assert_eq!(value["socketPath"], json!("/tmp/ta-cli.sock"));
}

/// Exercises the local IPC daemon-status contract; Windows currently aborts
/// inside the named-pipe contract harness before daemon-status logic is reached.
#[cfg(unix)]
#[test]
fn daemon_status_watch_count_one_json_smoke_outputs_poll_snapshot() {
    let socket_name = unique_socket_name("ta-cli-daemon-status-watch");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let expected_status = DaemonStatusResult {
        ready: true,
        daemon_instance_id: "daemon-1".to_string(),
        runtime_mode: DaemonRuntimeMode::Local,
        socket_path: "/tmp/ta-cli.sock".to_string(),
        log_path: "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl".to_string(),
        version: "0.0.1-test".to_string(),
    };

    let server_handle =
        spawn_daemon_server(listener, ServerResponse::Status(expected_status), None);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "status",
            "--watch",
            "--count",
            "1",
            "--json",
            "--socket",
            &socket_name,
        ])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["state"], json!("reachable"));
    assert_eq!(value["status"]["ready"], json!(true));
    assert_eq!(value["status"]["runtimeMode"], json!("local"));
}

/// Exercises the local IPC daemon-status contract; Windows currently aborts
/// inside the named-pipe contract harness before daemon-status logic is reached.
#[cfg(unix)]
#[test]
fn daemon_status_watch_count_one_json_smoke_outputs_error_state() {
    let socket_name = unique_socket_name("ta-cli-daemon-status-watch-error");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let server_handle = spawn_daemon_server(
        listener,
        ServerResponse::Error(JsonRpcErrorObject::method_not_found(METHOD_DAEMON_STATUS)),
        None,
    );

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "status",
            "--watch",
            "--count",
            "1",
            "--json",
            "--socket",
            &socket_name,
        ])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("method not found: daemon.status"),
        "stderr should include the remote daemon warning, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["state"], json!("error"));
    assert_eq!(value["socketPath"], json!(socket_address.to_string()));
    assert!(
        value["error"]
            .as_str()
            .is_some_and(|message| message.contains("method not found: daemon.status"))
    );
}

/// Exercises the local IPC daemon-status contract; Windows currently aborts
/// inside the named-pipe contract harness before daemon-status logic is reached.
#[cfg(unix)]
#[test]
fn daemon_status_watch_count_one_json_outputs_unavailable_state_with_derived_log_path() {
    let socket_name = unique_socket_name("ta-cli-daemon-status-watch-unavailable");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let temp_root = std::env::temp_dir().join(format!(
        "ta-cli-daemon-status-watch-unavailable-{}",
        unique_id_suffix()
    ));
    let log_dir = temp_root.join("logs");
    fs::create_dir_all(&log_dir).expect("log dir should exist");

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "status",
            "--watch",
            "--count",
            "1",
            "--json",
            "--socket",
            &socket_name,
        ])
        .env(LOG_DIR_ENV_VAR, &log_dir)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["state"], json!("unavailable"));
    assert_eq!(value["socketPath"], json!(socket_address.to_string()));
    assert_eq!(
        value["logPath"],
        json!(log_dir.join("ta-daemon.log.jsonl").display().to_string())
    );
    assert!(
        value["error"]
            .as_str()
            .is_some_and(|message| message.contains(&socket_address.to_string())),
        "error should surface socket unavailability, got: {value:?}"
    );

    if let SocketAddress::Unix(path) = &socket_address {
        let _ = fs::remove_file(path);
    }
    let _ = fs::remove_dir_all(temp_root);
}

/// Exercises the local IPC daemon-status contract; Windows currently aborts
/// inside the named-pipe contract harness before daemon-status logic is reached.
#[cfg(unix)]
#[test]
fn daemon_status_human_smoke_outputs_readable_summary() {
    let socket_name = unique_socket_name("ta-cli-daemon-status-human");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let expected_status = DaemonStatusResult {
        ready: true,
        daemon_instance_id: "daemon-1".to_string(),
        runtime_mode: DaemonRuntimeMode::Local,
        socket_path: "/tmp/ta-cli.sock".to_string(),
        log_path: "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl".to_string(),
        version: "0.0.1-test".to_string(),
    };

    let server_handle =
        spawn_daemon_server(listener, ServerResponse::Status(expected_status), None);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "status", "--socket", &socket_name])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("daemon ready"));
    assert!(stdout.contains("mode: local"));
    assert!(stdout.contains("socket: /tmp/ta-cli.sock"));
}

/// Exercises the local IPC daemon-status contract; Windows currently aborts
/// inside the named-pipe contract harness before daemon-status logic is reached.
#[cfg(unix)]
#[test]
fn daemon_status_remote_error_surfaces_on_stderr_with_exit_code_one() {
    let socket_name = unique_socket_name("ta-cli-daemon-status-error");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let server_handle = spawn_daemon_server(
        listener,
        ServerResponse::Error(JsonRpcErrorObject::method_not_found(METHOD_DAEMON_STATUS)),
        None,
    );

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "status", "--socket", &socket_name])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("remote JSON-RPC error -32601: method not found: daemon.status"),
        "stderr should surface the remote daemon error, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Exercises the local IPC daemon-status contract; Windows currently aborts
/// inside the named-pipe contract harness before daemon-status logic is reached.
#[cfg(unix)]
#[test]
fn daemon_status_socket_unavailable_surfaces_socket_error() {
    let socket_name = unique_socket_name("ta-cli-daemon-status-unavailable");

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "status", "--socket", &socket_name])
        .output()
        .expect("ta-cli should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No such file or directory")
            || stderr.contains("Connection refused")
            || stderr.contains("failed to connect"),
        "stderr should surface socket-unavailable error, got: {stderr}"
    );
}

#[cfg(not(windows))]
#[test]
fn daemon_background_status_json_smoke_outputs_parseable_payload() {
    let socket_name = unique_socket_name("ta-cli-daemon-background-status");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let snapshot = fake_background_control_status(&socket_address.to_string());
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-background-status-online", &snapshot);
    let server_handle = spawn_control_status_server(listener, socket_address.clone());

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "background",
            "status",
            "--json",
            "--socket",
            &socket_name,
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");

    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["desiredMode"], json!("local"));
    assert!(value["backgroundOptIn"].is_boolean());
    assert!(value["actualMode"].is_string());
    assert!(value["transitionStatus"].is_string());
    assert!(value["reconcileRequired"].is_boolean());
    assert!(value["allowedActions"].is_array());
    assert!(value["message"].is_string());
    assert!(
        value["socketPath"]
            .as_str()
            .is_some_and(|path| !path.is_empty())
    );
    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.trim().is_empty(),
        "expected no bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_background_reconcile_uses_local_control_bootstrap() {
    let socket_name = unique_socket_name("ta-cli-daemon-background-reconcile");
    let snapshot = fake_background_control_status(&socket_name);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-background-reconcile-online", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "background",
            "reconcile",
            "--json",
            "--socket",
            &socket_name,
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert!(value["desiredMode"].is_string());
    assert!(value["actualMode"].is_string());

    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::Reconcile.as_str()
        )]),
        "expected reconcile bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_background_enable_uses_local_control_bootstrap() {
    let socket_name = unique_socket_name("ta-cli-daemon-background-enable-online");
    let snapshot = fake_background_control_status(&socket_name);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-background-enable-online", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "background",
            "enable",
            "--json",
            "--socket",
            &socket_name,
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["desiredMode"], json!("background"));
    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::EnableBackground.as_str()
        )]),
        "expected enable-background bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_background_disable_uses_local_control_bootstrap() {
    let socket_name = unique_socket_name("ta-cli-daemon-background-disable-online");
    let snapshot = fake_background_control_status(&socket_name);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-background-disable-online", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "background",
            "disable",
            "--json",
            "--socket",
            &socket_name,
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert!(value["desiredMode"].is_string());
    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::DisableBackground.as_str()
        )]),
        "expected disable-background bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_background_reconcile_uses_local_control_bootstrap_when_socket_is_unavailable() {
    let socket_name = unique_socket_name("ta-cli-daemon-background-reconcile-unavailable");
    let socket_path = ServerConfig::local_default("ta-daemon-test", &socket_name)
        .socket_address
        .to_string();
    let snapshot = fake_background_control_status(&socket_path);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-background-reconcile-unavailable", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "background",
            "reconcile",
            "--json",
            "--socket",
            &socket_name,
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected reconcile to use local bootstrap"
    );

    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::Reconcile.as_str()
        )]),
        "expected reconcile bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_background_enable_uses_local_control_bootstrap_when_socket_is_unavailable() {
    let socket_name = unique_socket_name("ta-cli-daemon-background-enable-unavailable");
    let socket_path = ServerConfig::local_default("ta-daemon-test", &socket_name)
        .socket_address
        .to_string();
    let snapshot = fake_background_control_status(&socket_path);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-background-enable-unavailable", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "background",
            "enable",
            "--json",
            "--socket",
            &socket_name,
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected enable to use local bootstrap"
    );

    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::EnableBackground.as_str()
        )]),
        "expected enable-background bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_background_disable_uses_local_control_bootstrap_when_socket_is_unavailable() {
    let socket_name = unique_socket_name("ta-cli-daemon-background-disable-unavailable");
    let socket_path = ServerConfig::local_default("ta-daemon-test", &socket_name)
        .socket_address
        .to_string();
    let snapshot = fake_background_control_status(&socket_path);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-background-disable-unavailable", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "background",
            "disable",
            "--json",
            "--socket",
            &socket_name,
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected disable to use local bootstrap"
    );

    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::DisableBackground.as_str()
        )]),
        "expected disable-background bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_start_uses_bootstrap_subcommand_when_socket_is_unavailable() {
    let socket_name = unique_socket_name("ta-cli-daemon-start-bootstrap");
    let socket_path = ServerConfig::local_default("ta-daemon-test", &socket_name)
        .socket_address
        .to_string();
    let snapshot = fake_background_control_status(&socket_path);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-start-bootstrap", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "start",
            "--json",
            "--socket",
            &socket_name,
            "--timeout-ms",
            "50",
            "--interval-ms",
            "5",
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");

    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["started"], json!(true));
    assert_eq!(value["alreadyRunning"], json!(false));
    assert_eq!(value["socketPath"], json!(socket_path));
    assert_eq!(value["version"], json!("0.0.1-test"));

    let invocations =
        fs::read_to_string(&invocation_log).expect("fake daemon invocation log should exist");
    assert!(
        invocations.contains(&format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::Start.as_str()
        )),
        "expected bootstrap start invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[cfg(not(windows))]
#[test]
fn daemon_start_returns_already_running_when_daemon_is_reachable() {
    let socket_name = unique_socket_name("ta-cli-daemon-start-online");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let expected_status = DaemonStatusResult {
        ready: true,
        daemon_instance_id: "daemon-1".to_string(),
        runtime_mode: DaemonRuntimeMode::Local,
        socket_path: socket_address.to_string(),
        log_path: "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl".to_string(),
        version: "0.0.1-test".to_string(),
    };
    let snapshot = fake_background_control_status(&socket_address.to_string());
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-start-online", &snapshot);
    let server_handle =
        spawn_daemon_server(listener, ServerResponse::Status(expected_status), None);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "start",
            "--json",
            "--socket",
            &socket_name,
            "--timeout-ms",
            "50",
            "--interval-ms",
            "5",
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["started"], json!(false));
    assert_eq!(value["alreadyRunning"], json!(true));

    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.trim().is_empty(),
        "expected no bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[cfg(not(windows))]
#[test]
fn daemon_start_waits_for_reachable_not_ready_daemon_without_bootstrap() {
    let socket_name = unique_socket_name("ta-cli-daemon-start-not-ready");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let not_ready_status = DaemonStatusResult {
        ready: false,
        daemon_instance_id: "daemon-1".to_string(),
        runtime_mode: DaemonRuntimeMode::Local,
        socket_path: socket_address.to_string(),
        log_path: "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl".to_string(),
        version: "0.0.1-test".to_string(),
    };
    let ready_status = DaemonStatusResult {
        ready: true,
        ..not_ready_status.clone()
    };
    let server_handle =
        spawn_finite_daemon_status_sequence_server(listener, vec![not_ready_status, ready_status]);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "start",
            "--json",
            "--socket",
            &socket_name,
            "--timeout-ms",
            "100",
            "--interval-ms",
            "5",
        ])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["started"], json!(false));
    assert_eq!(value["alreadyRunning"], json!(true));
}

#[test]
fn daemon_background_status_unavailable_surfaces_socket_error_without_bootstrap() {
    let socket_name = unique_socket_name("ta-cli-daemon-background-status-unavailable-stderr");
    let socket_path = ServerConfig::local_default("ta-daemon-test", &socket_name)
        .socket_address
        .to_string();
    let snapshot = fake_background_control_status(&socket_path);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-background-status-unavailable-stderr", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "background", "status", "--socket", &socket_name])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No such file or directory")
            || stderr.contains("Connection refused")
            || stderr.contains("failed to connect"),
        "stderr should surface socket-unavailable error, got: {stderr}"
    );

    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.trim().is_empty(),
        "expected no bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_background_status_does_not_bootstrap_when_socket_is_unavailable() {
    let socket_name = unique_socket_name("ta-cli-daemon-background-bootstrap");
    let socket_path = ServerConfig::local_default("ta-daemon-test", &socket_name)
        .socket_address
        .to_string();
    let snapshot = fake_background_control_status(&socket_path);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-background-bootstrap", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "background",
            "status",
            "--json",
            "--socket",
            &socket_name,
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        !output.status.success(),
        "expected failure when daemon background status socket is unavailable"
    );

    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.trim().is_empty(),
        "expected no bootstrap snapshot invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_stop_offline_background_runtime_uses_local_control_bootstrap() {
    let socket_name = unique_socket_name("ta-cli-daemon-stop-background");
    let socket_path = ServerConfig::local_default("ta-daemon-test", &socket_name)
        .socket_address
        .to_string();
    let snapshot = fake_background_control_status(&socket_path);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-stop-background", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "stop", "--json", "--socket", &socket_name])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");

    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["stopping"], json!(true));

    let invocations =
        fs::read_to_string(&invocation_log).expect("fake daemon invocation log should exist");
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::Stop.as_str()
        )]),
        "expected only bootstrap stop invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_stop_offline_local_runtime_uses_local_control_bootstrap() {
    let socket_name = unique_socket_name("ta-cli-daemon-stop-local-offline");
    let socket_path = ServerConfig::local_default("ta-daemon-test", &socket_name)
        .socket_address
        .to_string();
    let snapshot = json!({
        "backgroundOptIn": false,
        "desiredMode": "local",
        "actualMode": "local",
        "transitionStatus": "idle",
        "reconcileRequired": false,
        "allowedActions": ["stop", "enableBackground"],
        "errorCode": null,
        "message": "Local mode is the desired runtime.",
        "pendingTransition": null,
        "socketPath": socket_path,
        "logPath": "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl",
        "daemonVersion": "0.0.1-test",
        "protocolVersion": "2026-04-stage2"
    });
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-stop-local-offline", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "stop", "--json", "--socket", &socket_name])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["stopping"], json!(true));

    let invocations =
        fs::read_to_string(&invocation_log).expect("fake daemon invocation log should exist");
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::Stop.as_str()
        )]),
        "expected only bootstrap stop invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_stop_offline_foreign_runtime_is_rejected() {
    let socket_name = unique_socket_name("ta-cli-daemon-stop-foreign-offline");
    let socket_path = ServerConfig::local_default("ta-daemon-test", &socket_name)
        .socket_address
        .to_string();
    let snapshot = json!({
        "backgroundOptIn": false,
        "desiredMode": "local",
        "actualMode": "foreign",
        "transitionStatus": "idle",
        "reconcileRequired": false,
        "allowedActions": [],
        "errorCode": "externalRuntime",
        "message": "Foreign runtime is active.",
        "pendingTransition": null,
        "socketPath": socket_path,
        "logPath": "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl",
        "daemonVersion": "0.0.1-test",
        "protocolVersion": "2026-04-stage2"
    });
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-stop-foreign-offline", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "stop", "--json", "--socket", &socket_name])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("refusing to stop a foreign runtime"),
        "stderr should surface foreign-runtime deny, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let invocations =
        fs::read_to_string(&invocation_log).expect("fake daemon invocation log should exist");
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::Stop.as_str()
        )]),
        "expected only bootstrap stop invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_stop_uses_local_control_bootstrap_when_daemon_is_reachable() {
    let socket_name = unique_socket_name("ta-cli-daemon-stop-online");
    let snapshot = fake_background_control_status(&socket_name);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-stop-online", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "stop", "--json", "--socket", &socket_name])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");

    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["stopping"], json!(true));

    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::Stop.as_str()
        )]),
        "expected stop bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_restart_fails_fast_when_control_status_socket_is_unavailable() {
    let socket_name = unique_socket_name("ta-cli-daemon-restart-unavailable");
    let socket_path = ServerConfig::local_default("ta-daemon-test", &socket_name)
        .socket_address
        .to_string();
    let snapshot = fake_background_control_status(&socket_path);
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-restart-unavailable", &snapshot);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "restart",
            "--json",
            "--socket",
            &socket_name,
            "--timeout-ms",
            "50",
            "--interval-ms",
            "5",
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    assert!(
        !output.status.success(),
        "expected restart to fail when control status socket is unavailable"
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No such file or directory")
            || stderr.contains("Connection refused")
            || stderr.contains("failed to connect"),
        "stderr should surface socket-unavailable error, got: {stderr}"
    );

    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.trim().is_empty(),
        "expected no bootstrap or handoff invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[cfg(not(windows))]
#[test]
fn daemon_restart_reachable_background_runtime_uses_local_control_bootstrap_stop_then_start() {
    let socket_name = unique_socket_name("ta-cli-daemon-restart-online");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let snapshot = fake_background_control_status(&socket_address.to_string());
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-restart-online", &snapshot);
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let server_handle = spawn_control_status_server(listener, socket_address.clone());

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "restart",
            "--json",
            "--socket",
            &socket_name,
            "--timeout-ms",
            "100",
            "--interval-ms",
            "5",
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");

    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["restarted"], json!(true));
    assert_eq!(value["wasRunning"], json!(true));
    assert_eq!(value["socketPath"], json!(socket_address.to_string()));
    assert_eq!(value["version"], json!("0.0.1-test"));

    let invocations =
        fs::read_to_string(&invocation_log).expect("fake daemon invocation log should exist");
    assert!(
        invocations.lines().eq([
            format!(
                "{} {}",
                RuntimeControlBootstrapCommand::SUBCOMMAND,
                RuntimeControlBootstrapCommand::Stop.as_str()
            ),
            format!(
                "{} {}",
                RuntimeControlBootstrapCommand::SUBCOMMAND,
                RuntimeControlBootstrapCommand::Start.as_str()
            ),
        ]),
        "expected bootstrap stop then start invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[cfg(not(windows))]
#[test]
fn daemon_restart_reachable_stopped_runtime_skips_stop_and_only_bootstraps_start() {
    let socket_name = unique_socket_name("ta-cli-daemon-restart-stopped");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let snapshot = fake_background_control_status(&socket_address.to_string());
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-restart-stopped", &snapshot);
    let server_handle = spawn_control_status_stopped_server(listener, socket_address.clone());

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "restart",
            "--json",
            "--socket",
            &socket_name,
            "--timeout-ms",
            "100",
            "--interval-ms",
            "5",
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["restarted"], json!(true));
    assert_eq!(value["wasRunning"], json!(false));

    let invocations =
        fs::read_to_string(&invocation_log).expect("fake daemon invocation log should exist");
    assert!(
        invocations.lines().eq([format!(
            "{} {}",
            RuntimeControlBootstrapCommand::SUBCOMMAND,
            RuntimeControlBootstrapCommand::Start.as_str()
        )]),
        "expected only bootstrap start invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[cfg(not(windows))]
#[test]
fn daemon_start_times_out_when_reachable_daemon_never_becomes_ready() {
    let socket_name = unique_socket_name("ta-cli-daemon-start-timeout");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let not_ready_status = DaemonStatusResult {
        ready: false,
        daemon_instance_id: "daemon-1".to_string(),
        runtime_mode: DaemonRuntimeMode::Local,
        socket_path: socket_address.to_string(),
        log_path: "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl".to_string(),
        version: "0.0.1-test".to_string(),
    };
    let snapshot = fake_background_control_status(&socket_address.to_string());
    let (temp_root, daemon_binary, invocation_log) =
        create_fake_daemon_binary("daemon-start-timeout", &snapshot);
    let server_handle = spawn_daemon_status_sequence_server(listener, vec![not_ready_status]);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "start",
            "--socket",
            &socket_name,
            "--timeout-ms",
            "20",
            "--interval-ms",
            "5",
        ])
        .env("TAUGENTIC_DAEMON_BINARY", &daemon_binary)
        .env("TA_FAKE_DAEMON_LOG", &invocation_log)
        .output()
        .expect("ta-cli should run");

    stop_status_sequence_server(&socket_address);
    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("daemon did not become ready within"),
        "stderr should surface timeout, got: {stderr}"
    );
    assert!(
        stderr.contains("/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl"),
        "stderr should include daemon log path, got: {stderr}"
    );

    let invocations = fs::read_to_string(&invocation_log).unwrap_or_default();
    assert!(
        invocations.trim().is_empty(),
        "expected no bootstrap invocation, got: {invocations}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

/// Exercises daemon-wait through the daemon-status local IPC harness; Windows
/// currently aborts inside the named-pipe contract harness before command logic is reached.
#[cfg(unix)]
#[test]
fn daemon_wait_json_smoke_polls_until_ready() {
    let socket_name = unique_socket_name("ta-cli-daemon-wait-ready");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let not_ready_status = DaemonStatusResult {
        ready: false,
        daemon_instance_id: "daemon-1".to_string(),
        runtime_mode: DaemonRuntimeMode::Local,
        socket_path: "/tmp/ta-cli.sock".to_string(),
        log_path: "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl".to_string(),
        version: "0.0.1-test".to_string(),
    };
    let ready_status = DaemonStatusResult {
        ready: true,
        ..not_ready_status.clone()
    };

    let server_handle =
        spawn_finite_daemon_status_sequence_server(listener, vec![not_ready_status, ready_status]);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "wait",
            "--json",
            "--socket",
            &socket_name,
            "--timeout-ms",
            "500",
            "--interval-ms",
            "10",
        ])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["ready"], json!(true));
    assert!(value["waitedMs"].is_number());
    assert_eq!(value["socketPath"], json!("/tmp/ta-cli.sock"));
}

/// Exercises daemon-wait through the daemon-status local IPC harness; Windows
/// currently aborts inside the named-pipe contract harness before command logic is reached.
#[cfg(unix)]
#[test]
fn daemon_wait_times_out_when_status_never_becomes_ready() {
    let socket_name = unique_socket_name("ta-cli-daemon-wait-timeout");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let not_ready_status = DaemonStatusResult {
        ready: false,
        daemon_instance_id: "daemon-1".to_string(),
        runtime_mode: DaemonRuntimeMode::Local,
        socket_path: "/tmp/ta-cli.sock".to_string(),
        log_path: "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl".to_string(),
        version: "0.0.1-test".to_string(),
    };

    let server_handle = spawn_daemon_status_sequence_server(listener, vec![not_ready_status]);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "wait",
            "--socket",
            &socket_name,
            "--timeout-ms",
            "20",
            "--interval-ms",
            "5",
        ])
        .output()
        .expect("ta-cli should run");

    stop_status_sequence_server(&socket_address);
    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("daemon did not become ready within"),
        "stderr should surface timeout, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl"),
        "stderr should include daemon log path, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn daemon_wait_times_out_when_socket_is_unavailable() {
    let socket_name = unique_socket_name("ta-cli-daemon-wait-unavailable");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let temp_root = std::env::temp_dir().join(format!(
        "ta-cli-daemon-wait-unavailable-{}",
        unique_id_suffix()
    ));
    let log_dir = temp_root.join("logs");
    fs::create_dir_all(&log_dir).expect("log dir should exist");

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "wait",
            "--socket",
            &socket_name,
            "--timeout-ms",
            "20",
            "--interval-ms",
            "5",
        ])
        .env(LOG_DIR_ENV_VAR, &log_dir)
        .output()
        .expect("ta-cli should run");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("daemon did not become ready within"),
        "stderr should surface timeout, got: {stderr}"
    );
    assert!(
        stderr.contains(&socket_address.to_string()),
        "stderr should include socket path, got: {stderr}"
    );
    assert!(
        stderr.contains(&log_dir.join("ta-daemon.log.jsonl").display().to_string()),
        "stderr should include derived daemon log path, got: {stderr}"
    );

    if let SocketAddress::Unix(path) = &socket_address {
        let _ = fs::remove_file(path);
    }
    let _ = fs::remove_dir_all(temp_root);
}

#[cfg(not(windows))]
#[test]
fn daemon_logs_uses_status_log_path_when_daemon_is_reachable() {
    let socket_name = unique_socket_name("ta-cli-daemon-logs-status");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let temp_root =
        std::env::temp_dir().join(format!("ta-cli-daemon-logs-status-{}", unique_id_suffix()));
    fs::create_dir_all(&temp_root).expect("temp root should exist");
    let log_path = temp_root.join("daemon.log.jsonl");
    fs::write(&log_path, "line-a\nline-b\nline-c\n").expect("log file should write");

    let expected_status = DaemonStatusResult {
        ready: true,
        daemon_instance_id: "daemon-1".to_string(),
        runtime_mode: DaemonRuntimeMode::Local,
        socket_path: socket_address.to_string(),
        log_path: log_path.display().to_string(),
        version: "0.0.1-test".to_string(),
    };

    let server_handle =
        spawn_daemon_server(listener, ServerResponse::Status(expected_status), None);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args([
            "daemon",
            "logs",
            "--tail",
            "2",
            "--json",
            "--socket",
            &socket_name,
        ])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["path"], json!(log_path.display().to_string()));
    assert_eq!(value["contents"], json!("line-b\nline-c"));
    assert_eq!(value["lines"], json!(2));
    assert_eq!(value["truncated"], json!(true));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn daemon_logs_uses_socket_derived_log_path_when_daemon_is_unavailable() {
    let socket_name = unique_socket_name("ta-cli-daemon-logs-unavailable");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let temp_root = std::env::temp_dir().join(format!(
        "ta-cli-daemon-logs-unavailable-{}",
        unique_id_suffix()
    ));
    let log_dir = temp_root.join("logs");
    fs::create_dir_all(&log_dir).expect("log dir should exist");
    let log_path = log_dir.join("ta-daemon.log.jsonl");
    fs::write(&log_path, "only-line\n").expect("log file should write");

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "logs", "--json", "--socket", &socket_name])
        .env(LOG_DIR_ENV_VAR, &log_dir)
        .output()
        .expect("ta-cli should run");

    let expected_path = log_path.display().to_string();
    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .expect("stdout should contain JSON");
    assert_eq!(value["path"], json!(expected_path));
    assert_eq!(value["contents"], json!("only-line"));
    assert_eq!(value["lines"], json!(1));
    assert_eq!(value["truncated"], json!(false));

    if let SocketAddress::Unix(path) = &socket_address {
        let _ = fs::remove_file(path);
    }
    let _ = fs::remove_dir_all(temp_root);
}

#[cfg(not(windows))]
#[test]
fn daemon_logs_reports_missing_log_file() {
    let socket_name = unique_socket_name("ta-cli-daemon-logs-missing");
    let socket_address = ServerConfig::local_default("ta-daemon-test", &socket_name).socket_address;
    let listener = bind_listener(&socket_address).expect("listener should bind");
    let missing_path =
        std::env::temp_dir().join(format!("ta-cli-missing-log-{}.jsonl", unique_id_suffix()));
    let expected_status = DaemonStatusResult {
        ready: true,
        daemon_instance_id: "daemon-1".to_string(),
        runtime_mode: DaemonRuntimeMode::Local,
        socket_path: socket_address.to_string(),
        log_path: missing_path.display().to_string(),
        version: "0.0.1-test".to_string(),
    };
    let server_handle =
        spawn_daemon_server(listener, ServerResponse::Status(expected_status), None);

    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(["daemon", "logs", "--json", "--socket", &socket_name])
        .output()
        .expect("ta-cli should run");

    server_handle.join().expect("server thread should complete");
    cleanup_socket_address(&socket_address);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(&format!(
            "daemon log file not found at {}",
            missing_path.display()
        )),
        "stderr should surface missing log file, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_help_surface(args: &[&str], expected_fragments: &[&str]) {
    let output = Command::cargo_bin("ta-cli")
        .expect("ta-cli binary should build")
        .args(args)
        .output()
        .expect("ta-cli should run");

    assert!(
        output.status.success(),
        "expected success, stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");

    let stdout = String::from_utf8_lossy(&output.stdout);
    for fragment in expected_fragments {
        assert!(
            stdout.contains(fragment),
            "stdout should contain {fragment:?}, got: {stdout}"
        );
    }
}

#[cfg(unix)]
fn spawn_daemon_server(
    listener: SocketListener,
    response: ServerResponse,
    cleanup_address: Option<SocketAddress>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut stream = listener
            .accept()
            .expect("listener should accept one client");
        let mut request_line = String::new();
        {
            let mut reader = BufReader::new(&mut stream);
            reader
                .read_line(&mut request_line)
                .expect("request should read");
        }

        let request = match JsonLineCodec
            .decode_message(&request_line)
            .expect("request should decode")
        {
            JsonRpcMessage::Request(request) => request,
            other => panic!("expected request, got {other:?}"),
        };

        let response = match response {
            ServerResponse::Status(status) => {
                let _: DaemonStatusParams = parse_params(&request).expect("params should parse");
                assert_eq!(request.method, METHOD_DAEMON_STATUS);
                JsonRpcMessage::Response(JsonRpcResponse::new(
                    request.id,
                    serde_json::to_value(status).expect("status should serialize"),
                ))
            }
            ServerResponse::Error(error) => {
                let _: DaemonStatusParams = parse_params(&request).expect("params should parse");
                assert_eq!(request.method, METHOD_DAEMON_STATUS);
                JsonRpcMessage::Error(JsonRpcError::new(Some(request.id), error))
            }
        };
        let line = JsonLineCodec
            .encode_message(&response)
            .expect("response should encode");
        stream
            .write_all(line.as_bytes())
            .expect("response should write");
        stream.flush().expect("response should flush");
        if let Some(address) = cleanup_address {
            cleanup_socket_address(&address);
        }
    })
}

#[cfg(unix)]
fn spawn_daemon_status_sequence_server(
    listener: SocketListener,
    responses: Vec<DaemonStatusResult>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let fallback_status = responses
            .last()
            .cloned()
            .expect("status sequence should include at least one response");
        let mut responses = responses.into_iter();
        loop {
            let mut stream = listener
                .accept()
                .expect("listener should accept one client per status request");
            let mut request_line = String::new();
            {
                let mut reader = BufReader::new(&mut stream);
                reader
                    .read_line(&mut request_line)
                    .expect("request should read");
            }

            let request = match JsonLineCodec
                .decode_message(&request_line)
                .expect("request should decode")
            {
                JsonRpcMessage::Request(request) => request,
                other => panic!("expected request, got {other:?}"),
            };
            if request.method == TEST_SERVER_SHUTDOWN_METHOD {
                break;
            }
            let _: DaemonStatusParams = parse_params(&request).expect("params should parse");
            assert_eq!(request.method, METHOD_DAEMON_STATUS);
            let status = responses.next().unwrap_or_else(|| fallback_status.clone());

            let response = JsonRpcMessage::Response(JsonRpcResponse::new(
                request.id,
                serde_json::to_value(status).expect("status should serialize"),
            ));
            let line = JsonLineCodec
                .encode_message(&response)
                .expect("response should encode");
            stream
                .write_all(line.as_bytes())
                .expect("response should write");
            stream.flush().expect("response should flush");
        }
    })
}

#[cfg(unix)]
fn spawn_finite_daemon_status_sequence_server(
    listener: SocketListener,
    responses: Vec<DaemonStatusResult>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        for status in responses {
            let mut stream = listener
                .accept()
                .expect("listener should accept one client per status request");
            let mut request_line = String::new();
            {
                let mut reader = BufReader::new(&mut stream);
                reader
                    .read_line(&mut request_line)
                    .expect("request should read");
            }

            let request = match JsonLineCodec
                .decode_message(&request_line)
                .expect("request should decode")
            {
                JsonRpcMessage::Request(request) => request,
                other => panic!("expected request, got {other:?}"),
            };
            let _: DaemonStatusParams = parse_params(&request).expect("params should parse");
            assert_eq!(request.method, METHOD_DAEMON_STATUS);

            let response = JsonRpcMessage::Response(JsonRpcResponse::new(
                request.id,
                serde_json::to_value(status).expect("status should serialize"),
            ));
            let line = JsonLineCodec
                .encode_message(&response)
                .expect("response should encode");
            stream
                .write_all(line.as_bytes())
                .expect("response should write");
            stream.flush().expect("response should flush");
        }
    })
}

fn spawn_session_list_server(
    listener: SocketListener,
    sessions: Vec<SessionSummary>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut stream = listener
            .accept()
            .expect("listener should accept one client");
        let mut reader = BufReader::new(&mut stream);

        let initialize = read_request(&mut reader);
        assert_eq!(initialize.method, METHOD_DAEMON_INITIALIZE);
        let _: ta_protocol::wire::DaemonInitializeParams =
            parse_params(&initialize).expect("initialize params should parse");
        write_response(&mut reader, initialize_ok_response(initialize.id));

        let request = read_request(&mut reader);
        let _: ListSessionsQuery = parse_params(&request).expect("params should parse");
        assert_eq!(request.method, METHOD_DAEMON_SESSION_LIST);
        write_response(
            &mut reader,
            JsonRpcResponse::new(
                request.id,
                serde_json::to_value(sessions).expect("sessions should serialize"),
            ),
        );
    })
}

fn spawn_control_status_server(
    listener: SocketListener,
    socket_address: SocketAddress,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut stream = listener
            .accept()
            .expect("listener should accept one client");
        let mut request_line = String::new();
        {
            let mut reader = BufReader::new(&mut stream);
            reader
                .read_line(&mut request_line)
                .expect("request should read");
        }

        let request = match JsonLineCodec
            .decode_message(&request_line)
            .expect("request should decode")
        {
            JsonRpcMessage::Request(request) => request,
            other => panic!("expected request, got {other:?}"),
        };
        let _: DaemonStatusParams = parse_params(&request).expect("params should parse");
        assert_eq!(request.method, METHOD_DAEMON_CONTROL_STATUS);

        let response = JsonRpcMessage::Response(JsonRpcResponse::new(
            request.id,
            json!({
                "backgroundOptIn": false,
                "desiredMode": "local",
                "actualMode": "local",
                "transitionStatus": "idle",
                "reconcileRequired": false,
                "allowedActions": ["stop", "enableBackground"],
                "errorCode": null,
                "message": "Local runtime is healthy.",
                "pendingTransition": null,
                "socketPath": socket_address.to_string(),
                "logPath": "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl",
                "daemonVersion": "0.0.1-test",
                "protocolVersion": "2026-04-stage2"
            }),
        ));
        let line = JsonLineCodec
            .encode_message(&response)
            .expect("response should encode");
        stream
            .write_all(line.as_bytes())
            .expect("response should write");
        stream.flush().expect("response should flush");
    })
}

fn spawn_control_status_stopped_server(
    listener: SocketListener,
    socket_address: SocketAddress,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut stream = listener
            .accept()
            .expect("listener should accept one client");
        let mut request_line = String::new();
        {
            let mut reader = BufReader::new(&mut stream);
            reader
                .read_line(&mut request_line)
                .expect("request should read");
        }

        let request = match JsonLineCodec
            .decode_message(&request_line)
            .expect("request should decode")
        {
            JsonRpcMessage::Request(request) => request,
            other => panic!("expected request, got {other:?}"),
        };
        let _: DaemonStatusParams = parse_params(&request).expect("params should parse");
        assert_eq!(request.method, METHOD_DAEMON_CONTROL_STATUS);

        let response = JsonRpcMessage::Response(JsonRpcResponse::new(
            request.id,
            json!({
                "backgroundOptIn": false,
                "desiredMode": "local",
                "actualMode": "stopped",
                "transitionStatus": "idle",
                "reconcileRequired": false,
                "allowedActions": ["start", "enableBackground"],
                "errorCode": null,
                "message": "Daemon is stopped.",
                "pendingTransition": null,
                "socketPath": socket_address.to_string(),
                "logPath": "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl",
                "daemonVersion": null,
                "protocolVersion": "2026-04-stage2"
            }),
        ));
        let line = JsonLineCodec
            .encode_message(&response)
            .expect("response should encode");
        stream
            .write_all(line.as_bytes())
            .expect("response should write");
        stream.flush().expect("response should flush");
        drop(stream);
        cleanup_socket_address(&socket_address);
    })
}

fn spawn_session_open_server(
    listener: SocketListener,
    expected_title: &str,
    expected_workspace_path: &str,
    expected_trust_acknowledged: bool,
    session: SessionSummary,
) -> JoinHandle<()> {
    let expected_title = expected_title.to_string();
    let expected_workspace_path = expected_workspace_path.to_string();
    thread::spawn(move || {
        let mut stream = listener
            .accept()
            .expect("listener should accept one persistent client");
        let mut reader = BufReader::new(&mut stream);

        let initialize = read_request(&mut reader);
        assert_eq!(initialize.method, METHOD_DAEMON_INITIALIZE);
        let _: ta_protocol::wire::DaemonInitializeParams =
            parse_params(&initialize).expect("initialize params should parse");
        write_response(&mut reader, initialize_ok_response(initialize.id));

        let request = read_request(&mut reader);
        let params: DaemonSessionOpenParams = parse_params(&request).expect("params should parse");
        assert_eq!(request.method, METHOD_DAEMON_SESSION_OPEN);
        assert_eq!(params.title, expected_title);
        assert_eq!(
            params.workspace,
            WorkspaceSelector::ByPath {
                path: ta_protocol::wire::WorkspacePath::from_canonical_wire_value(
                    expected_workspace_path
                )
                .expect("expected workspace path should be valid"),
                trust_acknowledged: expected_trust_acknowledged,
            },
            "session.open must propagate workspace selector from CLI",
        );
        write_response(
            &mut reader,
            JsonRpcResponse::new(
                request.id,
                serde_json::to_value(DaemonSessionOpenResult {
                    session,
                    latest_cursor: None,
                    session_authority: test_session_authority(),
                })
                .expect("session result should serialize"),
            ),
        );
    })
}

fn spawn_run_start_server(listener: SocketListener) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut stream = listener
            .accept()
            .expect("listener should accept one persistent client");
        let mut reader = BufReader::new(&mut stream);

        let initialize = read_request(&mut reader);
        assert_eq!(initialize.method, METHOD_DAEMON_INITIALIZE);
        let params: ta_protocol::wire::DaemonInitializeParams =
            parse_params(&initialize).expect("initialize params should parse");
        assert_eq!(params.protocol_version, DAEMON_PROTOCOL_VERSION);
        assert_eq!(
            params.capabilities,
            DaemonClientCapabilities {
                notifications: true,
                event_subscriptions: true,
            }
        );
        write_response(&mut reader, initialize_ok_response(initialize.id));

        let attach = read_request(&mut reader);
        assert_eq!(attach.method, METHOD_DAEMON_SESSION_ATTACH);
        let attach_params: DaemonSessionAttachParams =
            parse_params(&attach).expect("attach params should parse");
        assert_eq!(attach_params.session_id.as_str(), "session-1");
        assert_eq!(
            attach_params.session_authority.as_str(),
            TEST_SESSION_AUTHORITY
        );
        write_response(
            &mut reader,
            JsonRpcResponse::new(
                attach.id,
                serde_json::to_value(DaemonSessionAttachResult {
                    session: SessionSummary {
                        id: SessionId::new("session-1").expect("session id"),
                        title: "Build daemon app server".to_string(),
                        status: SessionStatus::Idle,
                        next_run_selection: SessionNextRunSelection::Unselected,
                    },
                    latest_cursor: None,
                    session_authority: SessionAuthority::new(
                        "session-authority-2session-authority-2".to_string(),
                    )
                    .expect("session authority"),
                })
                .expect("attach result should serialize"),
            ),
        );

        let start = read_request(&mut reader);
        assert_eq!(start.method, METHOD_DAEMON_RUN_START);
        let start_params: StartRunCommand =
            parse_params(&start).expect("run.start params should parse");
        assert_eq!(start_params.objective, "Ship app server hard cut");
        assert_eq!(
            start_params.selection,
            AgentRuntimeSelection {
                runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                auth_profile_id: Some(
                    AuthProfileId::new("profile-codex-test").expect("auth profile id"),
                ),
                model_id: Some(AgentRuntimeModelId::new("gpt-5.6-sol").expect("model id")),
            }
        );
        write_response(
            &mut reader,
            JsonRpcResponse::new(
                start.id,
                serde_json::to_value(RunSummary {
                    id: RunId::new("run-1").expect("run id"),
                    runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                        .expect("runtime profile id"),
                    objective: "Ship app server hard cut".to_string(),
                    status: RunStatus::WaitingForApproval,
                })
                .expect("run summary should serialize"),
            ),
        );
    })
}

fn spawn_approval_decide_server(listener: SocketListener) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut stream = listener
            .accept()
            .expect("listener should accept one persistent client");
        let mut reader = BufReader::new(&mut stream);

        let initialize = read_request(&mut reader);
        assert_eq!(initialize.method, METHOD_DAEMON_INITIALIZE);
        let params: ta_protocol::wire::DaemonInitializeParams =
            parse_params(&initialize).expect("initialize params should parse");
        assert_eq!(params.protocol_version, DAEMON_PROTOCOL_VERSION);
        write_response(&mut reader, initialize_ok_response(initialize.id));

        let attach = read_request(&mut reader);
        assert_eq!(attach.method, METHOD_DAEMON_SESSION_ATTACH);
        let attach_params: DaemonSessionAttachParams =
            parse_params(&attach).expect("attach params should parse");
        assert_eq!(attach_params.session_id.as_str(), "session-1");
        assert_eq!(
            attach_params.session_authority.as_str(),
            TEST_SESSION_AUTHORITY
        );
        write_response(
            &mut reader,
            JsonRpcResponse::new(
                attach.id,
                serde_json::to_value(DaemonSessionAttachResult {
                    session: SessionSummary {
                        id: SessionId::new("session-1").expect("session id"),
                        title: "Build daemon app server".to_string(),
                        status: SessionStatus::Running,
                        next_run_selection: SessionNextRunSelection::Unselected,
                    },
                    latest_cursor: None,
                    session_authority: SessionAuthority::new(
                        "session-authority-2session-authority-2".to_string(),
                    )
                    .expect("session authority"),
                })
                .expect("attach result should serialize"),
            ),
        );

        let decide = read_request(&mut reader);
        assert_eq!(decide.method, METHOD_DAEMON_APPROVAL_DECIDE);
        let decide_params: DaemonApprovalDecideParams =
            parse_params(&decide).expect("approval.decide params should parse");
        assert_eq!(decide_params.approval_id.as_str(), "approval-1");
        assert_eq!(decide_params.decision, ApprovalDecision::Approved);
        assert_eq!(decide_params.commentary, None);
        write_response(
            &mut reader,
            JsonRpcResponse::new(
                decide.id,
                serde_json::to_value(DaemonApprovalDecideResult {
                    run: RunSummary {
                        id: RunId::new("run-1").expect("run id"),
                        runtime_profile_id: RuntimeProfileId::new("runtime-codex-safe")
                            .expect("runtime profile id"),
                        objective: "Ship app server hard cut".to_string(),
                        status: RunStatus::Running,
                    },
                })
                .expect("approval decision result should serialize"),
            ),
        );
    })
}

fn read_request(reader: &mut BufReader<&mut ta_jsonrpc::SocketConnection>) -> JsonRpcRequest {
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .expect("request should read");
    match JsonLineCodec
        .decode_message(&request_line)
        .expect("request should decode")
    {
        JsonRpcMessage::Request(request) => request,
        other => panic!("expected request, got {other:?}"),
    }
}

fn write_response(
    reader: &mut BufReader<&mut ta_jsonrpc::SocketConnection>,
    response: JsonRpcResponse,
) {
    let line = JsonLineCodec
        .encode_message(&JsonRpcMessage::Response(response))
        .expect("response should encode");
    reader
        .get_mut()
        .write_all(line.as_bytes())
        .expect("response should write");
    reader.get_mut().flush().expect("response should flush");
}

fn initialize_ok_response(request_id: RequestId) -> JsonRpcResponse {
    JsonRpcResponse::new(
        request_id,
        serde_json::to_value(ta_protocol::wire::DaemonInitializeResult {
            daemon_instance_id: "daemon-1".to_string(),
            daemon_version: "0.0.1-test".to_string(),
            client_credential: TEST_CLIENT_CREDENTIAL.to_string(),
            protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
            capabilities: ta_protocol::wire::DaemonServerCapabilities {
                notifications: true,
                event_subscriptions: true,
            },
        })
        .expect("initialize result should serialize"),
    )
}

fn stop_status_sequence_server(address: &SocketAddress) {
    let mut stream = connect_socket(address).expect("test control connection should connect");
    let line = JsonLineCodec
        .encode_message(&JsonRpcMessage::Request(JsonRpcRequest::new(
            RequestId::Integer(9_999),
            TEST_SERVER_SHUTDOWN_METHOD,
            Some(json!({})),
        )))
        .expect("shutdown request should encode");
    stream
        .write_all(line.as_bytes())
        .expect("shutdown request should write");
    stream.flush().expect("shutdown request should flush");
}

fn unique_socket_name(prefix: &str) -> String {
    format!("{prefix}-{}", unique_id_suffix())
}

fn seed_local_session_authority(
    socket_address: &SocketAddress,
    client_name: &str,
    session_id: &str,
) {
    let path = session_authority_path(socket_address, client_name, session_id);
    fs::create_dir_all(
        path.parent()
            .expect("session authority path should have parent"),
    )
    .expect("session authority dir should exist");
    fs::write(&path, TEST_SESSION_AUTHORITY).expect("session authority should persist");
}

fn seed_local_client_credential(socket_address: &SocketAddress, client_name: &str) {
    let path = client_credential_path(socket_address, client_name);
    fs::create_dir_all(
        path.parent()
            .expect("client credential path should have parent"),
    )
    .expect("client credential dir should exist");
    fs::write(&path, TEST_CLIENT_CREDENTIAL).expect("client credential should persist");
}

fn client_credential_path(socket_address: &SocketAddress, client_name: &str) -> PathBuf {
    let client_storage_key = stable_storage_key(client_name.trim());
    match socket_address {
        SocketAddress::Unix(path) => {
            let socket_name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .expect("socket path should have file stem");
            path.parent()
                .expect("socket path should have parent")
                .join("taugentic-client-credentials")
                .join(socket_name)
                .join(format!("{client_storage_key}.credential"))
        }
        SocketAddress::NamedPipe(name) => std::env::temp_dir()
            .join("taugentic-client-credentials")
            .join(name)
            .join(format!("{client_storage_key}.credential")),
    }
}

fn session_authority_path(
    socket_address: &SocketAddress,
    client_name: &str,
    session_id: &str,
) -> PathBuf {
    let client_storage_key = stable_storage_key(client_name.trim());
    let session_storage_key = stable_storage_key(session_id);
    match socket_address {
        SocketAddress::Unix(path) => {
            let socket_name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .expect("socket path should have file stem");
            path.parent()
                .expect("socket path should have parent")
                .join("taugentic-session-authorities")
                .join(socket_name)
                .join(client_storage_key)
                .join(format!("{session_storage_key}.authority"))
        }
        SocketAddress::NamedPipe(name) => std::env::temp_dir()
            .join("taugentic-session-authorities")
            .join(name)
            .join(client_storage_key)
            .join(format!("{session_storage_key}.authority")),
    }
}

fn stable_storage_key(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn test_session_authority() -> SessionAuthority {
    SessionAuthority::new(TEST_SESSION_AUTHORITY.to_string()).expect("session authority")
}

fn unique_id_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos()
}

fn fake_background_control_status(socket_path: &str) -> serde_json::Value {
    json!({
        "backgroundOptIn": true,
        "desiredMode": "background",
        "actualMode": "background",
        "transitionStatus": "idle",
        "reconcileRequired": false,
        "allowedActions": ["stop", "disableBackground"],
        "errorCode": null,
        "message": "Background mode is the desired runtime.",
        "pendingTransition": null,
        "socketPath": socket_path,
        "logPath": "/tmp/taugentic-daemon/ta-cli/ta-daemon.log.jsonl",
        "daemonVersion": "0.0.1-test",
        "protocolVersion": "2026-04-stage2"
    })
}

fn create_fake_daemon_binary(
    label: &str,
    snapshot: &serde_json::Value,
) -> (PathBuf, PathBuf, PathBuf) {
    let temp_root =
        std::env::temp_dir().join(format!("ta-cli-fake-daemon-{label}-{}", unique_id_suffix()));
    fs::create_dir_all(&temp_root).expect("fake daemon temp root should exist");
    let invocation_log = temp_root.join("invocations.log");
    let daemon_binary = temp_root.join(fake_daemon_binary_name());
    let snapshot_json = serde_json::to_string(snapshot).expect("snapshot JSON should serialize");
    let stop_detail = snapshot
        .get("actualMode")
        .and_then(|value| value.as_str())
        .filter(|mode| *mode == "foreign")
        .map(|_| "refusing to stop a foreign runtime");
    write_fake_daemon_program(&daemon_binary, &invocation_log, &snapshot_json, stop_detail);
    (temp_root, daemon_binary, invocation_log)
}

fn fake_daemon_binary_name() -> &'static str {
    if cfg!(windows) {
        "ta-daemon.cmd"
    } else {
        "ta-daemon"
    }
}

fn write_fake_daemon_program(
    path: &Path,
    invocation_log: &Path,
    snapshot_json: &str,
    stop_detail: Option<&str>,
) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let stop_branch = if let Some(detail) = stop_detail {
            format!(
                "if [ \"$1\" = \"{bootstrap_subcommand}\" ] && [ \"$2\" = \"{bootstrap_stop}\" ]; then\n  echo '{detail}' >&2\n  exit 1\nfi\n",
                bootstrap_subcommand = RuntimeControlBootstrapCommand::SUBCOMMAND,
                bootstrap_stop = RuntimeControlBootstrapCommand::Stop.as_str(),
                detail = shell_single_quote_escape(detail),
            )
        } else {
            format!(
                "if [ \"$1\" = \"{bootstrap_subcommand}\" ] && [ \"$2\" = \"{bootstrap_stop}\" ]; then\n  printf '%s\\n' '{json}'\n  exit 0\nfi\n",
                bootstrap_subcommand = RuntimeControlBootstrapCommand::SUBCOMMAND,
                bootstrap_stop = RuntimeControlBootstrapCommand::Stop.as_str(),
                json = shell_single_quote_escape(snapshot_json),
            )
        };
        let script = format!(
            "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$*\" >> \"{log}\"\nif [ \"$1\" = \"{bootstrap_subcommand}\" ] && [ \"$2\" = \"{bootstrap_start}\" ]; then\n  printf '%s\\n' '{json}'\n  exit 0\nfi\nif [ \"$1\" = \"{bootstrap_subcommand}\" ] && [ \"$2\" = \"{bootstrap_snapshot}\" ]; then\n  printf '%s\\n' '{json}'\n  exit 0\nfi\nif [ \"$1\" = \"{bootstrap_subcommand}\" ] && [ \"$2\" = \"{bootstrap_reconcile}\" ]; then\n  printf '%s\\n' '{json}'\n  exit 0\nfi\nif [ \"$1\" = \"{bootstrap_subcommand}\" ] && [ \"$2\" = \"{bootstrap_enable_background}\" ]; then\n  printf '%s\\n' '{json}'\n  exit 0\nfi\nif [ \"$1\" = \"{bootstrap_subcommand}\" ] && [ \"$2\" = \"{bootstrap_disable_background}\" ]; then\n  printf '%s\\n' '{json}'\n  exit 0\nfi\n{stop_branch}echo \"unexpected args: $*\" >&2\nexit 1\n",
            log = shell_single_quote_escape(invocation_log.to_string_lossy().as_ref()),
            json = shell_single_quote_escape(snapshot_json),
            bootstrap_subcommand = RuntimeControlBootstrapCommand::SUBCOMMAND,
            bootstrap_start = RuntimeControlBootstrapCommand::Start.as_str(),
            bootstrap_snapshot = RuntimeControlBootstrapCommand::Snapshot.as_str(),
            bootstrap_reconcile = RuntimeControlBootstrapCommand::Reconcile.as_str(),
            bootstrap_enable_background = RuntimeControlBootstrapCommand::EnableBackground.as_str(),
            bootstrap_disable_background =
                RuntimeControlBootstrapCommand::DisableBackground.as_str(),
            stop_branch = stop_branch,
        );
        fs::write(path, script).expect("fake daemon script should write");
        let mut permissions = fs::metadata(path)
            .expect("fake daemon script metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("fake daemon script should be executable");
    }

    #[cfg(windows)]
    {
        let stop_branch = if let Some(detail) = stop_detail {
            format!(
                "if \"%~1\"==\"{bootstrap_subcommand}\" if \"%~2\"==\"{bootstrap_stop}\" (\r\n  echo {detail} 1>&2\r\n  exit /b 1\r\n)\r\n",
                bootstrap_subcommand = RuntimeControlBootstrapCommand::SUBCOMMAND,
                bootstrap_stop = RuntimeControlBootstrapCommand::Stop.as_str(),
                detail = detail,
            )
        } else {
            format!(
                "if \"%~1\"==\"{bootstrap_subcommand}\" if \"%~2\"==\"{bootstrap_stop}\" (\r\n  echo({json}\r\n  exit /b 0\r\n)\r\n",
                bootstrap_subcommand = RuntimeControlBootstrapCommand::SUBCOMMAND,
                bootstrap_stop = RuntimeControlBootstrapCommand::Stop.as_str(),
                json = snapshot_json,
            )
        };
        let script = format!(
            "@echo off\r\nsetlocal\r\necho %*>>\"{log}\"\r\nif \"%~1\"==\"{bootstrap_subcommand}\" if \"%~2\"==\"{bootstrap_start}\" (\r\n  echo({json}\r\n  exit /b 0\r\n)\r\nif \"%~1\"==\"{bootstrap_subcommand}\" if \"%~2\"==\"{bootstrap_snapshot}\" (\r\n  echo({json}\r\n  exit /b 0\r\n)\r\nif \"%~1\"==\"{bootstrap_subcommand}\" if \"%~2\"==\"{bootstrap_reconcile}\" (\r\n  echo({json}\r\n  exit /b 0\r\n)\r\nif \"%~1\"==\"{bootstrap_subcommand}\" if \"%~2\"==\"{bootstrap_enable_background}\" (\r\n  echo({json}\r\n  exit /b 0\r\n)\r\nif \"%~1\"==\"{bootstrap_subcommand}\" if \"%~2\"==\"{bootstrap_disable_background}\" (\r\n  echo({json}\r\n  exit /b 0\r\n)\r\n{stop_branch}echo unexpected args: %* 1>&2\r\nexit /b 1\r\n",
            log = invocation_log.display(),
            json = snapshot_json,
            bootstrap_subcommand = RuntimeControlBootstrapCommand::SUBCOMMAND,
            bootstrap_start = RuntimeControlBootstrapCommand::Start.as_str(),
            bootstrap_snapshot = RuntimeControlBootstrapCommand::Snapshot.as_str(),
            bootstrap_reconcile = RuntimeControlBootstrapCommand::Reconcile.as_str(),
            bootstrap_enable_background = RuntimeControlBootstrapCommand::EnableBackground.as_str(),
            bootstrap_disable_background =
                RuntimeControlBootstrapCommand::DisableBackground.as_str(),
            stop_branch = stop_branch,
        );
        fs::write(path, script).expect("fake daemon batch file should write");
    }
}

#[cfg(unix)]
fn shell_single_quote_escape(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

fn cleanup_socket_address(address: &SocketAddress) {
    if let SocketAddress::Unix(path) = address {
        let _ = fs::remove_file(path);
    }
}
