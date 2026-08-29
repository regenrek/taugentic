use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ta_protocol::wire::{
    METHOD_DAEMON_TERMINAL_ATTACH, METHOD_DAEMON_TERMINAL_CLOSE, METHOD_DAEMON_TERMINAL_DETACH,
    METHOD_DAEMON_TERMINAL_EVENT, METHOD_DAEMON_TERMINAL_INPUT, METHOD_DAEMON_TERMINAL_RESIZE,
    METHOD_DAEMON_TERMINAL_SPAWN, TerminalAttachResult, TerminalCloseResult, TerminalDetachResult,
    TerminalEventParams, TerminalInputResult, TerminalResizeResult, TerminalSessionStatus,
    TerminalSpawnResult, TerminalStreamEvent, WorkspacePath,
};

use super::*;

#[test]
fn daemon_terminal_routes_stream_and_detach_without_closing_the_shell() {
    let state = boot(test_config());
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    let (connection_runtime, mut outbound_rx) =
        JsonRpcConnectionRuntime::new(77, DEFAULT_OUTBOUND_QUEUE_DEPTH);
    let session = connection_runtime.session();
    let session_state = Arc::new(Mutex::new(DaemonRpcSessionState::default()));
    initialize_client(
        &state,
        &shutdown_requested,
        &session,
        &session_state,
        TEST_CLIENT_NAME,
    );
    let principal_id = session_state
        .lock()
        .expect("session state")
        .principal_id
        .clone()
        .expect("initialized principal");
    let root = tempfile::tempdir().expect("workspace tempdir");
    let (project_id, snapshot) = state
        .app
        .open_project(
            &principal_id,
            WorkspacePath::canonicalize_existing(root.path()).expect("workspace path"),
            true,
        )
        .expect("project should open");
    let workspace_id = snapshot
        .projects
        .iter()
        .find(|project| project.id == project_id)
        .and_then(|project| project.workspace_ids.first())
        .cloned()
        .expect("project workspace");

    let spawned: TerminalSpawnResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(701),
                method: METHOD_DAEMON_TERMINAL_SPAWN.to_string(),
                params: Some(serde_json::json!({
                    "projectId": project_id,
                    "workspaceId": workspace_id,
                    "rows": 24,
                    "cols": 80,
                    "userApproved": true,
                })),
            },
        )
        .expect("terminal spawn route should succeed"),
    )
    .expect("terminal spawn response");
    let terminal_id = spawned.terminal.id.clone();

    let attached: TerminalAttachResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(702),
                method: METHOD_DAEMON_TERMINAL_ATTACH.to_string(),
                params: Some(serde_json::json!({ "terminalId": terminal_id })),
            },
        )
        .expect("terminal attach route should succeed"),
    )
    .expect("terminal attach response");
    assert_eq!(attached.terminal.status, TerminalSessionStatus::Running);
    for action in connection_runtime.take_after_response_actions() {
        action();
    }

    let input: TerminalInputResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(703),
                method: METHOD_DAEMON_TERMINAL_INPUT.to_string(),
                params: Some(serde_json::json!({
                    "terminalId": terminal_id,
                    "dataBase64": BASE64.encode(b"printf 'rpc-terminal-ready\\n'\n"),
                })),
            },
        )
        .expect("terminal input route should succeed"),
    )
    .expect("terminal input response");
    assert!(input.accepted_bytes > 0);
    let event = recv_terminal_event(&mut outbound_rx);
    assert!(matches!(event.event, TerminalStreamEvent::Output { .. }));

    let resized: TerminalResizeResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(704),
                method: METHOD_DAEMON_TERMINAL_RESIZE.to_string(),
                params: Some(serde_json::json!({
                    "terminalId": terminal_id,
                    "rows": 40,
                    "cols": 120,
                })),
            },
        )
        .expect("terminal resize route should succeed"),
    )
    .expect("terminal resize response");
    assert_eq!((resized.terminal.rows, resized.terminal.cols), (40, 120));

    let detached: TerminalDetachResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(705),
                method: METHOD_DAEMON_TERMINAL_DETACH.to_string(),
                params: Some(serde_json::json!({ "terminalId": terminal_id })),
            },
        )
        .expect("terminal detach route should succeed"),
    )
    .expect("terminal detach response");
    assert!(detached.detached);

    let closed: TerminalCloseResult = serde_json::from_value(
        handle_request(
            &state,
            &shutdown_requested,
            &session,
            &session_state,
            JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: crate::RequestId::Integer(706),
                method: METHOD_DAEMON_TERMINAL_CLOSE.to_string(),
                params: Some(serde_json::json!({ "terminalId": terminal_id })),
            },
        )
        .expect("terminal close route should succeed"),
    )
    .expect("terminal close response");
    assert_eq!(closed.terminal.status, TerminalSessionStatus::Exited);
    session.close();
}

fn recv_terminal_event(
    outbound_rx: &mut tokio::sync::mpsc::Receiver<JsonRpcMessage>,
) -> TerminalEventParams {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match outbound_rx.try_recv() {
            Ok(JsonRpcMessage::Notification(notification))
                if notification.method == METHOD_DAEMON_TERMINAL_EVENT =>
            {
                return serde_json::from_value(
                    notification
                        .params
                        .expect("terminal event params should exist"),
                )
                .expect("terminal event should deserialize");
            }
            Ok(other) => panic!("expected daemon.terminal.event notification, got {other:?}"),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("timed out waiting for terminal event: {error:?}"),
        }
    }
}
