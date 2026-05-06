use crate::support::*;

#[test]
fn real_daemon_status_reports_ready_payload() {
    let socket_name = unique_name("ta-daemon-it-status");
    let mut daemon = ManagedDaemon::spawn(&socket_name);

    let status = daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status");

    assert!(status.ready);
    assert_eq!(status.runtime_mode, DaemonRuntimeMode::Local);
    assert_eq!(status.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(
        status.socket_path,
        daemon.client().config().socket_address.to_string()
    );
    assert_eq!(
        status.log_path,
        daemon
            .log_dir
            .join("ta-daemon.log.jsonl")
            .display()
            .to_string()
    );
}

#[test]
fn real_daemon_writes_startup_logs_to_resolved_json_log_file() {
    let socket_name = unique_name("ta-daemon-it-startup-log");
    let mut daemon = ManagedDaemon::spawn(&socket_name);

    let status = daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before startup log assertions");
    assert!(
        status.log_path.ends_with("ta-daemon.log.jsonl"),
        "daemon.status should surface the canonical base log path, got: {}",
        status.log_path
    );

    let log_path = wait_for_daily_log_file(&daemon.log_dir, "ta-daemon.log.jsonl")
        .expect("startup log file should be created");
    let entries = wait_for_log_entries(
        &log_path,
        &["observability initialized", "daemon boot complete"],
    )
    .expect("startup log should contain observability and boot records");

    let init_entry = find_log_entry(&entries, "observability initialized")
        .expect("startup log should contain observability initialized");
    assert_eq!(init_entry["fields"]["service.name"], json!("ta-daemon"));
    assert_eq!(init_entry["fields"]["log.effective_format"], json!("json"));
    assert_eq!(init_entry["fields"]["log.stderr"], json!(false));
    assert_eq!(
        init_entry["fields"]["log.file"],
        json!("ta-daemon.log.jsonl")
    );

    let boot_entry = find_log_entry(&entries, "daemon boot complete")
        .expect("startup log should contain daemon boot complete");
    assert_eq!(
        boot_entry["fields"]["message"],
        json!("daemon boot complete")
    );
    assert_eq!(
        boot_entry["fields"]["daemon.instance_id"],
        json!(status.daemon_instance_id)
    );
    assert!(
        boot_entry["fields"]["socket.address"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "daemon boot log should include socket.address, got: {boot_entry:?}"
    );
}

#[test]
fn real_daemon_returns_json_rpc_method_not_found_for_unknown_method() {
    let socket_name = unique_name("ta-daemon-it-unknown");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before method-not-found assertions");

    let client = daemon.client();
    let error = client
        .send_request(JsonRpcRequest::new(
            RequestId::Integer(42),
            "unknown.method",
            Some(json!({})),
        ))
        .expect_err("unknown method should return a remote JSON-RPC error");

    let JsonRpcClientError::Remote(error) = error else {
        panic!("expected remote JSON-RPC error, got {error:?}");
    };

    assert_eq!(error.error.code, METHOD_NOT_FOUND_ERROR_CODE);
    assert!(error.error.message.contains("unknown.method"));
}

#[test]
fn real_daemon_rejects_removed_public_daemon_stop_method() {
    let socket_name = unique_name("ta-daemon-it-stop-method-removed");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before daemon.stop assertions");

    let client = daemon.client();
    let error = client
        .send_request(JsonRpcRequest::new(
            RequestId::Integer(43),
            "daemon.stop",
            Some(json!({ "controlToken": "unused" })),
        ))
        .expect_err("removed public daemon.stop should return method-not-found");

    let JsonRpcClientError::Remote(error) = error else {
        panic!("expected remote JSON-RPC error, got {error:?}");
    };

    assert_eq!(error.error.code, METHOD_NOT_FOUND_ERROR_CODE);
    assert!(error.error.message.contains("daemon.stop"));
}

#[test]
fn real_daemon_control_status_succeeds_without_initialize_on_fresh_connection() {
    let socket_name = unique_name("ta-daemon-it-control-status");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before daemon.control.status assertions");

    let client = daemon.client();
    let result: DaemonControlStatusResult = client
        .call(METHOD_DAEMON_CONTROL_STATUS, &DaemonStatusParams {})
        .expect("daemon.control.status should succeed on a fresh connection without initialize");

    assert_eq!(result.protocol_version, DAEMON_PROTOCOL_VERSION);
    assert_eq!(
        result.daemon_version,
        Some(env!("CARGO_PKG_VERSION").to_string())
    );
    assert!(!result.message.is_empty());
    assert!(!result.socket_path.is_empty());
    assert!(!result.log_path.is_empty());
}

#[test]
fn real_daemon_mutating_public_control_methods_are_not_exposed() {
    let socket_name = unique_name("ta-daemon-it-control-public-disabled");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon.wait_for_status().expect(
        "real daemon should answer daemon.status before public control mutation assertions",
    );

    let client = daemon.client();
    for method in [
        "daemon.background.enable",
        "daemon.background.disable",
        "daemon.control.reconcile",
        "daemon.control.stop",
    ] {
        let error = client
            .send_request(JsonRpcRequest::new(
                RequestId::Integer(100),
                method,
                Some(json!({})),
            ))
            .expect_err("mutating public control method should return method-not-found");

        let JsonRpcClientError::Remote(error) = error else {
            panic!("expected remote JSON-RPC error, got {error:?}");
        };

        assert_eq!(error.error.code, METHOD_NOT_FOUND_ERROR_CODE);
        assert_eq!(error.error.message, format!("method not found: {method}"));
    }
}

#[test]
fn real_daemon_remote_websocket_accepts_bearer_auth_after_launch_projection() {
    let socket_name = unique_name("ta-daemon-it-remote-ws");
    let remote_bind = reserve_tcp_address();
    let auth_token = "0123456789abcdef0123456789abcdef";
    let mut daemon = ManagedDaemon::spawn_with_env(
        &socket_name,
        &[
            (DAEMON_REMOTE_WS_ENABLED_ENV_VAR, "1"),
            (DAEMON_REMOTE_WS_BIND_ENV_VAR, &remote_bind),
            (DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR, auth_token),
        ],
    );

    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before remote websocket assertions");

    let uri: tungstenite::http::Uri = format!("ws://{remote_bind}/rpc")
        .parse()
        .expect("remote websocket uri should parse");
    let request = tungstenite::ClientRequestBuilder::new(uri)
        .with_header("Authorization", format!("Bearer {auth_token}"));
    let (mut socket, _response) =
        tungstenite::connect(request).expect("remote websocket should accept bearer auth");

    let request = JsonRpcMessage::Request(JsonRpcRequest::new(
        RequestId::Integer(1),
        METHOD_DAEMON_STATUS,
        Some(json!({})),
    ));
    let payload = serde_json::to_string(&request).expect("json-rpc request should serialize");
    socket
        .send(Message::Text(payload.into()))
        .expect("remote websocket request should send");

    let message = socket.read().expect("remote websocket should answer");
    let Message::Text(payload) = message else {
        panic!("expected text websocket response, got {message:?}");
    };
    let response: JsonRpcMessage =
        serde_json::from_str(&payload).expect("remote websocket response should parse");
    let JsonRpcMessage::Response(response) = response else {
        panic!("expected json-rpc response, got {response:?}");
    };
    let status: DaemonStatusResult =
        serde_json::from_value(response.result).expect("daemon.status result should deserialize");

    assert!(status.ready);
    assert_eq!(status.runtime_mode, DaemonRuntimeMode::Local);
}

#[test]
fn real_daemon_fails_fast_for_invalid_remote_websocket_config() {
    let socket_name = unique_name("ta-daemon-it-invalid-remote-ws");
    let output = spawn_daemon_with_env(
        &socket_name,
        &[
            (DAEMON_REMOTE_WS_ENABLED_ENV_VAR, "1"),
            (DAEMON_REMOTE_WS_BIND_ENV_VAR, "0.0.0.0:42321"),
        ],
    )
    .expect("daemon should exit with invalid remote websocket config");

    assert!(
        !output.status.success(),
        "daemon should fail fast for invalid remote websocket config"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("remote websocket bind address"),
        "stderr should mention invalid remote websocket bind address, got: {stderr}"
    );
    assert!(
        stderr.contains("loopback-only"),
        "stderr should mention loopback-only constraint, got: {stderr}"
    );
}

#[test]
fn real_daemon_fails_fast_for_missing_remote_websocket_auth_token() {
    let socket_name = unique_name("ta-daemon-it-missing-remote-auth");
    let output = spawn_daemon_with_env(
        &socket_name,
        &[
            (DAEMON_REMOTE_WS_ENABLED_ENV_VAR, "1"),
            (DAEMON_REMOTE_WS_BIND_ENV_VAR, "127.0.0.1:42321"),
        ],
    )
    .expect("daemon should exit when remote websocket auth token is missing");

    assert!(
        !output.status.success(),
        "daemon should fail fast when remote websocket auth token is missing"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("remote websocket auth token env var"),
        "stderr should mention missing remote websocket auth token, got: {stderr}"
    );
    assert!(
        stderr.contains(DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR),
        "stderr should name the missing auth token env var, got: {stderr}"
    );
}

#[test]
fn real_daemon_fails_fast_for_invalid_remote_websocket_auth_token_without_leaking_value() {
    let socket_name = unique_name("ta-daemon-it-invalid-remote-auth");
    let invalid_auth_token = "raw-secret";
    let output = spawn_daemon_with_env(
        &socket_name,
        &[
            (DAEMON_REMOTE_WS_ENABLED_ENV_VAR, "1"),
            (DAEMON_REMOTE_WS_BIND_ENV_VAR, "127.0.0.1:42321"),
            (DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR, invalid_auth_token),
        ],
    )
    .expect("daemon should exit when remote websocket auth token is invalid");

    assert!(
        !output.status.success(),
        "daemon should fail fast when remote websocket auth token is invalid"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("remote websocket auth token"),
        "stderr should mention invalid remote websocket auth token, got: {stderr}"
    );
    assert!(
        stderr.contains("at least 16"),
        "stderr should mention minimum token length, got: {stderr}"
    );
    assert!(
        stderr.contains(DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR),
        "stderr should name the invalid auth token env var, got: {stderr}"
    );
    assert!(
        !stderr.contains(invalid_auth_token),
        "stderr must not leak the invalid auth token value, got: {stderr}"
    );
}

#[test]
fn real_daemon_fails_fast_for_invalid_log_stderr_value_without_leaking_value() {
    let socket_name = unique_name("ta-daemon-it-invalid-log-stderr");
    let invalid_log_stderr = "raw-secret-token";
    let output = spawn_daemon_with_env(&socket_name, &[(LOG_STDERR_ENV_VAR, invalid_log_stderr)])
        .expect("daemon should exit when TAUGENTIC_LOG_STDERR is invalid");

    assert!(
        !output.status.success(),
        "daemon should fail fast when TAUGENTIC_LOG_STDERR is invalid"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid TAUGENTIC_LOG_STDERR value"),
        "stderr should mention invalid TAUGENTIC_LOG_STDERR, got: {stderr}"
    );
    assert!(
        stderr.contains("expected one of: true, false, 1, 0, yes, no, on, off"),
        "stderr should mention accepted bool values, got: {stderr}"
    );
    assert!(
        !stderr.contains("daemon boot complete"),
        "daemon must fail before boot completes, got: {stderr}"
    );
    assert!(
        !stderr.contains(invalid_log_stderr),
        "stderr must not leak the invalid TAUGENTIC_LOG_STDERR value, got: {stderr}"
    );
}

#[test]
fn real_daemon_fails_fast_for_invalid_log_format_value_without_leaking_value() {
    let socket_name = unique_name("ta-daemon-it-invalid-log-format");
    let invalid_log_format = "raw-secret-token";
    let output = spawn_daemon_with_env(&socket_name, &[(LOG_FORMAT_ENV_VAR, invalid_log_format)])
        .expect("daemon should exit when TAUGENTIC_LOG_FORMAT is invalid");

    assert!(
        !output.status.success(),
        "daemon should fail fast when TAUGENTIC_LOG_FORMAT is invalid"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid TAUGENTIC_LOG_FORMAT value"),
        "stderr should mention invalid TAUGENTIC_LOG_FORMAT, got: {stderr}"
    );
    assert!(
        stderr.contains("expected one of: pretty, json"),
        "stderr should mention accepted log formats, got: {stderr}"
    );
    assert!(
        !stderr.contains("daemon boot complete"),
        "daemon must fail before boot completes, got: {stderr}"
    );
    assert!(
        !stderr.contains(invalid_log_format),
        "stderr must not leak the invalid TAUGENTIC_LOG_FORMAT value, got: {stderr}"
    );
}

#[test]
fn real_daemon_fails_fast_for_invalid_remote_websocket_enable_flag_without_leaking_value() {
    let socket_name = unique_name("ta-daemon-it-invalid-remote-enable");
    let invalid_remote_enable = "raw-secret-token";
    let output = spawn_daemon_with_env(
        &socket_name,
        &[(DAEMON_REMOTE_WS_ENABLED_ENV_VAR, invalid_remote_enable)],
    )
    .expect("daemon should exit when TAUGENTIC_DAEMON_REMOTE_WS_ENABLED is invalid");

    assert!(
        !output.status.success(),
        "daemon should fail fast when TAUGENTIC_DAEMON_REMOTE_WS_ENABLED is invalid"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid remote websocket enable flag"),
        "stderr should mention invalid remote websocket enable flag, got: {stderr}"
    );
    assert!(
        stderr.contains(DAEMON_REMOTE_WS_ENABLED_ENV_VAR),
        "stderr should name TAUGENTIC_DAEMON_REMOTE_WS_ENABLED, got: {stderr}"
    );
    assert!(
        stderr.contains("expected 0/1/true/false"),
        "stderr should mention accepted enable values, got: {stderr}"
    );
    assert!(
        !stderr.contains("daemon boot complete"),
        "daemon must fail before boot completes, got: {stderr}"
    );
    assert!(
        !stderr.contains(invalid_remote_enable),
        "stderr must not leak the invalid remote enable value, got: {stderr}"
    );
}

#[test]
fn real_daemon_fails_fast_for_malformed_persisted_runtime_mode_without_leaking_value() {
    let socket_name = unique_name("ta-daemon-it-malformed-persisted-runtime-mode");
    let root_dir = test_temp_dir("ta-daemon-invalid-runtime-mode-config");
    let invalid_runtime_mode = "raw-secret-token";
    let runtime_mode_path = runtime_control_state_path_for_root(&root_dir);
    fs::create_dir_all(runtime_mode_path.parent().expect("parent should exist"))
        .expect("config dir should exist");
    fs::write(
        &runtime_mode_path,
        format!("{{ definitely {invalid_runtime_mode}"),
    )
    .expect("persisted runtime mode should write");
    assert!(
        runtime_mode_path.exists(),
        "persisted runtime mode file should exist at {}",
        runtime_mode_path.display()
    );

    let log_dir = root_dir.join("logs");
    fs::create_dir_all(&log_dir).expect("test log dir should exist");
    let mut command = Command::new(env!("CARGO_BIN_EXE_ta-daemon"));
    command
        .env(DAEMON_SOCKET_NAME_ENV_VAR, &socket_name)
        .env_remove(DAEMON_RUNTIME_MODE_ENV_VAR)
        .env(LOG_DIR_ENV_VAR, &log_dir)
        .env(LOG_STDERR_ENV_VAR, "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    apply_isolated_config_env(&mut command, &root_dir);
    let mut child = command
        .spawn()
        .expect("daemon should spawn for malformed persisted runtime mode test");
    let deadline = Instant::now() + STARTUP_TIMEOUT;

    let output = loop {
        if child
            .try_wait()
            .expect("failed to poll daemon process")
            .is_some()
        {
            break child
                .wait_with_output()
                .expect("failed to collect daemon output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("daemon should exit when persisted runtime mode file is malformed");
        }
        thread::sleep(POLL_INTERVAL);
    };

    let _ = fs::remove_dir_all(&root_dir);
    let _ = fs::remove_dir_all(&log_dir);

    assert!(
        !output.status.success(),
        "daemon should fail fast when persisted runtime mode file is malformed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid daemon runtime mode in persisted config"),
        "stderr should mention invalid persisted runtime mode, got: {stderr}"
    );
    assert!(
        stderr.contains("expected local or background"),
        "stderr should mention accepted runtime modes, got: {stderr}"
    );
    assert!(
        !stderr.contains("daemon boot complete"),
        "daemon must fail before boot completes, got: {stderr}"
    );
    assert!(
        !stderr.contains(invalid_runtime_mode),
        "stderr must not leak malformed persisted runtime mode contents, got: {stderr}"
    );
}
