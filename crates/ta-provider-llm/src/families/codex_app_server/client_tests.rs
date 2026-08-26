use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use ta_protocol::wire::{AuthMethodId, AuthProfileConnectionState, AuthProfileLoginMethod};

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

#[test]
#[cfg(unix)]
fn account_login_retains_early_completion_and_reads_the_profile_account() {
    let binary = write_script(
        "account-login",
        r#"#!/usr/bin/env python3
import json, os, sys
profile_ok = os.environ.get("CODEX_HOME", "").endswith("/profile-login-test")
for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialize":
        print(json.dumps({"id": msg["id"], "result": {}}), flush=True)
    elif method == "initialized":
        continue
    elif method == "account/login/start":
        if not profile_ok:
            print(json.dumps({"id": msg["id"], "error": {"code": -32602, "message": "profile scope missing"}}), flush=True)
            continue
        print(json.dumps({"method": "account/login/completed", "params": {"loginId": "login-test", "success": True, "error": None}}), flush=True)
        print(json.dumps({"id": msg["id"], "result": {"type": "chatgpt", "loginId": "login-test", "authUrl": "https://example.test/authorize"}}), flush=True)
    elif method == "account/read":
        print(json.dumps({"id": msg["id"], "result": {"account": {"type": "chatgpt", "email": "person@example.test", "planType": "pro"}, "requiresOpenaiAuth": True}}), flush=True)
    elif method == "account/logout":
        if "params" in msg:
            print(json.dumps({"id": msg["id"], "error": {"code": -32602, "message": "logout params must be omitted"}}), flush=True)
            continue
        print(json.dumps({"id": msg["id"], "result": {}}), flush=True)
"#,
    );
    let client = CodexAppServerClient::with_binary(binary);
    let profile_id = AuthProfileId::new("profile-login-test").expect("profile id");
    let mut session = client
        .start_control_session_for_profile(&profile_id)
        .expect("profile control session");

    let (login_id, auth_url) = session.start_chatgpt_login().expect("login start");
    assert_eq!(
        (login_id.as_str(), auth_url.as_str()),
        ("login-test", "https://example.test/authorize")
    );
    session
        .wait_for_chatgpt_login(&login_id)
        .expect("early completion notification");
    assert_eq!(
        session.read_chatgpt_account().expect("account read"),
        Some((
            Some("person@example.test".to_string()),
            Some("pro".to_string())
        ))
    );
    session.logout_account().expect("account logout");
}

#[test]
#[cfg(unix)]
fn auth_login_returns_the_browser_challenge_before_awaiting_completion() {
    let binary = write_script(
        "two-phase-account-login",
        r#"#!/usr/bin/env python3
import json, sys
for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get("method")
    if method == "initialize":
        print(json.dumps({"id": msg["id"], "result": {}}), flush=True)
    elif method == "initialized":
        continue
    elif method == "account/login/start":
        print(json.dumps({"id": msg["id"], "result": {"type": "chatgpt", "loginId": "two-phase-login", "authUrl": "https://example.test/authorize"}}), flush=True)
        print(json.dumps({"method": "account/login/completed", "params": {"loginId": "two-phase-login", "success": True, "error": None}}), flush=True)
    elif method == "account/read":
        print(json.dumps({"id": msg["id"], "result": {"account": {"type": "chatgpt", "email": "person@example.test", "planType": "pro"}, "requiresOpenaiAuth": True}}), flush=True)
"#,
    );
    let client = CodexAppServerClient::with_binary(binary);
    let profile_id = AuthProfileId::new("profile-two-phase-login").expect("profile id");
    let auth_method_id = AuthMethodId::new("codex-chatgpt").expect("auth method id");

    let started = crate::auth::codex_oauth::login(&client, &auth_method_id, &profile_id)
        .expect("login start");
    assert_eq!(
        started.auth_profile.connection_state,
        AuthProfileConnectionState::PendingLogin
    );
    let challenge = started.challenge.expect("browser challenge");
    assert_eq!(challenge.auth_profile_id, profile_id);
    assert_eq!(challenge.method, AuthProfileLoginMethod::Browser);
    assert_eq!(
        challenge.authorize_url.expect("authorize URL").as_str(),
        "https://example.test/authorize"
    );

    let completed =
        crate::auth::codex_oauth::complete_login(&profile_id).expect("login completion");
    assert_eq!(
        completed.auth_profile.connection_state,
        AuthProfileConnectionState::Connected
    );
    assert!(completed.challenge.is_none());
}
