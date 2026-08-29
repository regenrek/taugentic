use ta_protocol::wire::{
    CodeHostAccountListResult, METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT,
    METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT, METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL, METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE,
    METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST, METHOD_DAEMON_CODE_HOST_PUSH_APPLY,
    METHOD_DAEMON_CODE_HOST_PUSH_PREPARE, METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT,
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

fn repository() -> serde_json::Value {
    serde_json::json!({
        "provider": "gitHub",
        "host": "github.com",
        "owner": "example-owner",
        "name": "example-project",
    })
}

#[test]
fn code_host_rpc_routes_require_initialization_and_return_typed_account_results() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session = test_session();
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));

    let uninitialized = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        request(
            901,
            METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST,
            serde_json::json!({}),
        ),
    )
    .expect_err("code-host reads require daemon.initialize");
    assert_eq!(uninitialized.code, crate::INVALID_PARAMS_ERROR_CODE);

    initialize_client(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        TEST_CLIENT_NAME,
    );
    let result: CodeHostAccountListResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            request(
                902,
                METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST,
                serde_json::json!({}),
            ),
        )
        .expect("initialized code-host account list should succeed"),
    )
    .expect("account list result should be typed");
    assert!(result.accounts.is_empty());
}

#[test]
fn every_code_host_rpc_route_rejects_unknown_params_before_dispatch() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session = test_session();
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    let scoped = serde_json::json!({
        "projectId": "project-test",
        "workspaceId": "workspace-test",
        "accountId": "account-test",
        "repository": repository(),
    });
    let cases = [
        (METHOD_DAEMON_CODE_HOST_ACCOUNT_LIST, serde_json::json!({})),
        (
            METHOD_DAEMON_CODE_HOST_ACCOUNT_CONNECT,
            serde_json::json!({
                "provider": "gitHub",
                "displayName": "Local profile",
                "host": "github.com",
                "accessToken": "not-a-real-token",
            }),
        ),
        (
            METHOD_DAEMON_CODE_HOST_ACCOUNT_DISCONNECT,
            serde_json::json!({ "accountId": "account-test" }),
        ),
        (
            METHOD_DAEMON_CODE_HOST_REPOSITORY_CONTEXT,
            serde_json::json!({
                "projectId": "project-test",
                "workspaceId": "workspace-test",
            }),
        ),
        (
            METHOD_DAEMON_CODE_HOST_PUSH_PREPARE,
            serde_json::json!({
                "projectId": "project-test",
                "workspaceId": "workspace-test",
                "accountId": "account-test",
                "remoteName": "origin",
                "destinationBranch": "main",
            }),
        ),
        (
            METHOD_DAEMON_CODE_HOST_PUSH_APPLY,
            serde_json::json!({ "token": "push-token-test" }),
        ),
        (METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST, scoped.clone()),
        (METHOD_DAEMON_CODE_HOST_PULL_REQUEST_DETAIL, {
            let mut value = scoped.clone();
            value["number"] = serde_json::json!("1");
            value
        }),
        (
            METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ENSURE,
            serde_json::json!({
                "projectId": "project-test",
                "workspaceId": "workspace-test",
                "accountId": "account-test",
                "headRemoteName": "origin",
                "headBranch": "feature",
                "baseRemoteName": "upstream",
                "baseBranch": "main",
                "title": "Change",
                "body": "Description",
                "draft": false,
            }),
        ),
        (METHOD_DAEMON_CODE_HOST_PULL_REQUEST_CHECKS, {
            let mut value = scoped.clone();
            value["headSha"] = serde_json::json!("1111111111111111111111111111111111111111");
            value
        }),
        (METHOD_DAEMON_CODE_HOST_PULL_REQUEST_ACTIVITY, {
            let mut value = scoped.clone();
            value["number"] = serde_json::json!("1");
            value
        }),
        (METHOD_DAEMON_CODE_HOST_PULL_REQUEST_COMMENT_CREATE, {
            let mut value = scoped;
            value["number"] = serde_json::json!("1");
            value["body"] = serde_json::json!("Review comment");
            value
        }),
    ];

    for (index, (method, mut params)) in cases.into_iter().enumerate() {
        params["unexpected"] = serde_json::json!(true);
        let error = handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            request(910 + index as i64, method, params),
        )
        .expect_err("unknown code-host params must be rejected");
        assert_eq!(
            error.code,
            crate::INVALID_PARAMS_ERROR_CODE,
            "{method} accepted an unknown parameter"
        );
    }
}

#[test]
fn code_host_rpc_scopes_repository_before_resolving_credentials() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let session = test_session();
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    initialize_client(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        TEST_CLIENT_NAME,
    );

    let error = handle_request(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        request(
            930,
            METHOD_DAEMON_CODE_HOST_PULL_REQUEST_LIST,
            serde_json::json!({
                "projectId": "project-not-present",
                "workspaceId": "workspace-not-present",
                "accountId": "account-not-present",
                "repository": repository(),
            }),
        ),
    )
    .expect_err("repository scope should reject before credential access");
    assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
    assert_eq!(
        error.data,
        Some(serde_json::json!({ "code": "ProjectNotFound" }))
    );
}
