use ta_protocol::wire::{
    METHOD_DAEMON_THREAD_WORKSPACE_GET, METHOD_DAEMON_THREAD_WORKSPACE_UPDATE,
    ThreadWorkspaceResult,
};

use super::*;

fn request(id: i64, method: &str, params: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: crate::RequestId::Integer(id),
        method: method.to_string(),
        params: Some(params),
    }
}

fn initialized_session_state(
    owner_principal_id: &str,
    session_id: Option<SessionId>,
) -> Arc<Mutex<DaemonRpcSessionState>> {
    Arc::new(Mutex::new(DaemonRpcSessionState {
        initialized: true,
        client_name: Some(TEST_CLIENT_NAME.to_string()),
        client_credential: Some(TEST_CLIENT_CREDENTIAL.to_string()),
        principal_id: Some(owner_principal_id.to_string()),
        attached_session_id: session_id,
    }))
}

fn issue_test_principal_id(state: &BootstrapState) -> String {
    state
        .app
        .resolve_or_issue_session_principal(TEST_CLIENT_NAME, None)
        .expect("test principal should issue")
        .principal_id
}

fn open_session(state: &BootstrapState, owner_principal_id: &str) -> SessionId {
    state
        .app
        .open_session(
            TEST_CLIENT_NAME,
            owner_principal_id,
            &OpenSessionRequest {
                title: "Thread workspace RPC".to_string(),
                workspace_id: ta_store::default_test_workspace_id(),
            },
        )
        .expect("session should open")
        .id
        .clone()
}

#[test]
fn thread_workspace_request_parser_rejects_unknown_top_level_and_nested_fields() {
    for (method, params) in [
        (
            METHOD_DAEMON_THREAD_WORKSPACE_GET,
            serde_json::json!({ "unexpected": true }),
        ),
        (
            METHOD_DAEMON_THREAD_WORKSPACE_UPDATE,
            serde_json::json!({
                "mutation": { "kind": "goalSet", "value": "goal" },
                "unexpected": true,
            }),
        ),
        (
            METHOD_DAEMON_THREAD_WORKSPACE_UPDATE,
            serde_json::json!({
                "mutation": {
                    "kind": "goalSet",
                    "value": "goal",
                    "unexpected": true,
                },
            }),
        ),
    ] {
        let error = super::super::request::DaemonRpcRequest::parse(&request(1, method, params))
            .expect_err("unknown Thread Workspace fields must be rejected by the request parser");
        assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    }
}

#[test]
fn thread_workspace_routes_require_initialization_and_an_attached_session() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session = test_session();
    let uninitialized = Arc::new(Mutex::new(DaemonRpcSessionState::default()));

    for (id, method, params) in [
        (
            10,
            METHOD_DAEMON_THREAD_WORKSPACE_GET,
            serde_json::json!({}),
        ),
        (
            11,
            METHOD_DAEMON_THREAD_WORKSPACE_UPDATE,
            serde_json::json!({ "mutation": { "kind": "goalSet", "value": "goal" } }),
        ),
    ] {
        let error = handle_request(
            &state,
            &shutdown_requested,
            &session,
            &uninitialized,
            request(id, method, params),
        )
        .expect_err("Thread Workspace routes require daemon.initialize");
        assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
        assert!(error.message.contains("daemon.initialize"));
    }

    let owner_principal_id = issue_test_principal_id(&state);
    let unattached = initialized_session_state(&owner_principal_id, None);
    for (id, method, params) in [
        (
            12,
            METHOD_DAEMON_THREAD_WORKSPACE_GET,
            serde_json::json!({}),
        ),
        (
            13,
            METHOD_DAEMON_THREAD_WORKSPACE_UPDATE,
            serde_json::json!({ "mutation": { "kind": "goalSet", "value": "goal" } }),
        ),
    ] {
        let error = handle_request(
            &state,
            &shutdown_requested,
            &session,
            &unattached,
            request(id, method, params),
        )
        .expect_err("Thread Workspace routes require an attached session");
        assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
        assert!(error.message.contains("daemon.session.open"));
    }
}

#[test]
fn thread_workspace_rpc_is_attached_session_scoped_and_store_backed() {
    let state = boot(test_config());
    let owner_principal_id = issue_test_principal_id(&state);
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let attached_session_id = open_session(&state, &owner_principal_id);
    let other_session_id = open_session(&state, &owner_principal_id);
    let session_state =
        initialized_session_state(&owner_principal_id, Some(attached_session_id.clone()));
    let session = test_session();

    let fresh: ThreadWorkspaceResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            request(
                20,
                METHOD_DAEMON_THREAD_WORKSPACE_GET,
                serde_json::json!({}),
            ),
        )
        .expect("attached get should succeed"),
    )
    .expect("get result should be typed");
    assert_eq!(fresh.session_id, attached_session_id);
    assert!(fresh.goal.is_empty() && fresh.plan.is_empty() && fresh.notes.is_empty());
    assert!(fresh.recap.is_empty() && fresh.pins.is_empty() && fresh.work_log.is_empty());

    for (id, kind, value) in [
        (21, "goalSet", "goal"),
        (22, "planSet", "plan"),
        (23, "notesSet", "notes"),
        (24, "recapSet", "recap"),
    ] {
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            request(
                id,
                METHOD_DAEMON_THREAD_WORKSPACE_UPDATE,
                serde_json::json!({ "mutation": { "kind": kind, "value": value } }),
            ),
        )
        .expect("attached update should persist through the daemon store");
    }

    let projection: ThreadWorkspaceResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            request(
                25,
                METHOD_DAEMON_THREAD_WORKSPACE_GET,
                serde_json::json!({}),
            ),
        )
        .expect("later attached get should observe the store projection"),
    )
    .expect("get result should be typed");
    assert_eq!(projection.session_id, attached_session_id);
    assert_eq!(
        (
            projection.goal.as_str(),
            projection.plan.as_str(),
            projection.notes.as_str(),
            projection.recap.as_str()
        ),
        ("goal", "plan", "notes", "recap")
    );
    assert_eq!(projection.work_log.len(), 4);

    for (id, method, params) in [
        (
            26,
            METHOD_DAEMON_THREAD_WORKSPACE_GET,
            serde_json::json!({ "sessionId": other_session_id }),
        ),
        (
            27,
            METHOD_DAEMON_THREAD_WORKSPACE_UPDATE,
            serde_json::json!({
                "sessionId": other_session_id,
                "mutation": { "kind": "goalSet", "value": "other" },
            }),
        ),
    ] {
        let error = handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            request(id, method, params),
        )
        .expect_err("request params must not select another session");
        assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    }
}

#[test]
fn thread_workspace_invalid_durable_pin_uses_the_safe_store_error_mapping() {
    let state = boot(test_config());
    let owner_principal_id = issue_test_principal_id(&state);
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session_id = open_session(&state, &owner_principal_id);
    let session_state = initialized_session_state(&owner_principal_id, Some(session_id));
    let session = test_session();

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        request(
            30,
            METHOD_DAEMON_THREAD_WORKSPACE_UPDATE,
            serde_json::json!({
                "mutation": {
                    "kind": "pinAdded",
                    "pin": { "runId": "run-test", "cursor": { "sequence": "0" } },
                },
            }),
        ),
    )
    .expect_err("a pin must reference a durable cursor");
    assert_eq!(error.code, -32603);
}
