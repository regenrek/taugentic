use crate::support::*;

#[test]
fn second_real_daemon_on_same_socket_fails_without_breaking_first() {
    let socket_name = unique_name("ta-daemon-it-bind");
    let mut first = ManagedDaemon::spawn(&socket_name);
    first
        .wait_for_status()
        .expect("first daemon should bind and answer before conflict test");

    let output = spawn_conflicting_daemon(&socket_name)
        .expect("second daemon should fail quickly when the socket is already live");
    assert!(
        !output.status.success(),
        "conflicting daemon should exit non-zero"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to bind socket"),
        "stderr should report bind failure, got: {stderr}"
    );
    if cfg!(unix) {
        assert!(
            stderr.contains("daemon already running"),
            "unix bind failure should mention live socket protection, got: {stderr}"
        );
    }

    let status = first
        .wait_for_status()
        .expect("first daemon should remain healthy after the conflicting start");
    assert!(status.ready);
}

#[test]
fn persistent_daemon_subscribe_surfaces_history_gap_for_stale_or_ahead_cursor() {
    let socket_name = unique_name("ta-daemon-it-gap");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before subscribe gap assertions");

    let client = daemon.client();
    let mut stream =
        connect_socket(&client.config().socket_address).expect("persistent client should connect");
    configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
        .expect("persistent client should configure socket deadlines");
    let codec = JsonLineCodec;

    write_request(
        &codec,
        &mut stream,
        JsonRpcRequest::new(
            RequestId::Integer(1),
            METHOD_DAEMON_INITIALIZE,
            Some(
                serde_json::to_value(DaemonInitializeParams {
                    client_name: "ta-orchestrator-tests".to_string(),
                    client_credential: None,
                    client_version: "0.0.1".to_string(),
                    protocol_version: DAEMON_PROTOCOL_VERSION.to_string(),
                    capabilities: DaemonClientCapabilities {
                        notifications: true,
                        event_subscriptions: true,
                    },
                })
                .expect("initialize params should serialize"),
            ),
        ),
    );
    let initialize = read_response(&codec, &mut stream);
    assert_eq!(initialize.id, RequestId::Integer(1));
    let initialize_result: DaemonInitializeResult =
        serde_json::from_value(initialize.result).expect("initialize result should deserialize");

    let opened = open_session(
        &mut stream,
        RequestId::Integer(2),
        "Build daemon app server",
    );
    assert_eq!(
        opened.latest_cursor.as_ref().map(|cursor| cursor.sequence),
        Some(1)
    );
    assert_eq!(
        opened
            .latest_cursor
            .as_ref()
            .map(|cursor| cursor.daemon_instance_id.as_str()),
        Some(initialize_result.daemon_instance_id.as_str())
    );
    assert_eq!(
        opened
            .latest_cursor
            .as_ref()
            .map(|cursor| cursor.session_id.as_str()),
        Some(opened.session.id.as_str())
    );

    write_request(
        &codec,
        &mut stream,
        JsonRpcRequest::new(
            RequestId::Integer(3),
            METHOD_DAEMON_SUBSCRIBE,
            Some(json!({
                "kinds": [DaemonEventKind::Run],
                "afterCursor": {
                    "daemonInstanceId": "stale-daemon",
                    "sessionId": opened.session.id,
                    "sequence": "42"
                }
            })),
        ),
    );
    let subscribe = read_response(&codec, &mut stream);
    assert_eq!(subscribe.id, RequestId::Integer(3));
    let result: DaemonSubscribeResult =
        serde_json::from_value(subscribe.result).expect("subscribe result should deserialize");
    assert_eq!(
        result,
        DaemonSubscribeResult::HistoryGap {
            latest_cursor: opened.latest_cursor.clone(),
        }
    );
}

#[test]
fn persistent_connections_require_explicit_initialize_before_subscribe_and_after_reattach() {
    let socket_name = unique_name("ta-daemon-it-explicit-attach");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before explicit attach assertions");

    let client = daemon.client();
    let mut first_stream = connect_socket(&client.config().socket_address)
        .expect("first persistent client should connect");
    configure_connection_timeouts(&first_stream, Some(Duration::from_secs(5)))
        .expect("first persistent client should configure socket deadlines");

    let first_attach_error =
        subscribe_run_events_expect_invalid_params(&mut first_stream, RequestId::Integer(1));
    assert_eq!(first_attach_error.id, Some(RequestId::Integer(1)));
    assert_eq!(first_attach_error.error.code, INVALID_PARAMS_ERROR_CODE);
    assert_eq!(
        first_attach_error.error.message,
        "daemon.subscribe requires daemon.initialize first"
    );

    let first_initialize =
        initialize_named_session(&mut first_stream, RequestId::Integer(2), "desktop-main");
    let opened = open_session(
        &mut first_stream,
        RequestId::Integer(3),
        "Build daemon app server",
    );

    let first_subscribe = subscribe_run_events(&mut first_stream, RequestId::Integer(4));
    assert!(matches!(
        first_subscribe,
        DaemonSubscribeResult::Ready { .. }
    ));
    drop(first_stream);

    let mut reattach_stream =
        connect_socket(&client.config().socket_address).expect("reattach client should connect");
    configure_connection_timeouts(&reattach_stream, Some(Duration::from_secs(5)))
        .expect("reattach client should configure socket deadlines");

    let reattach_error =
        subscribe_run_events_expect_invalid_params(&mut reattach_stream, RequestId::Integer(4));
    assert_eq!(reattach_error.id, Some(RequestId::Integer(4)));
    assert_eq!(reattach_error.error.code, INVALID_PARAMS_ERROR_CODE);
    assert_eq!(
        reattach_error.error.message,
        "daemon.subscribe requires daemon.initialize first"
    );

    let reattach_initialize = initialize_named_session_with_credential(
        &mut reattach_stream,
        RequestId::Integer(5),
        "desktop-main",
        Some(&first_initialize.client_credential),
    );
    assert_eq!(
        reattach_initialize.daemon_instance_id,
        first_initialize.daemon_instance_id
    );
    assert_eq!(
        reattach_initialize.protocol_version,
        first_initialize.protocol_version
    );
    let attached = attach_session(
        &mut reattach_stream,
        RequestId::Integer(6),
        opened.session.id.clone(),
        opened.session_authority.clone(),
    );
    assert_eq!(attached.session.id, opened.session.id);

    let reattach_subscribe = subscribe_run_events(&mut reattach_stream, RequestId::Integer(7));
    assert!(matches!(
        reattach_subscribe,
        DaemonSubscribeResult::Ready { .. }
    ));
}

#[test]
fn daemon_owned_session_open_rejects_attach_from_another_initialized_connection() {
    let socket_name = unique_name("ta-daemon-it-session-open");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before session.open assertions");

    let client = daemon.client();

    let mut opener_stream =
        connect_socket(&client.config().socket_address).expect("session opener should connect");
    configure_connection_timeouts(&opener_stream, Some(Duration::from_secs(5)))
        .expect("session opener should configure socket deadlines");
    let opener_initialize =
        initialize_named_session(&mut opener_stream, RequestId::Integer(1), "desktop-main");
    let opened = open_session(
        &mut opener_stream,
        RequestId::Integer(2),
        "Build daemon app server",
    );
    assert!(opened.session.id.as_str().starts_with("session-"));
    assert_eq!(opened.session.title, "Build daemon app server");
    assert_eq!(
        opened.session.status,
        ta_protocol::wire::SessionStatus::Idle
    );
    assert_eq!(
        opened.latest_cursor.as_ref().map(|cursor| cursor.sequence),
        Some(1)
    );
    assert_eq!(
        opened
            .latest_cursor
            .as_ref()
            .map(|cursor| cursor.session_id.as_str()),
        Some(opened.session.id.as_str())
    );

    let mut attach_stream = connect_socket(&client.config().socket_address)
        .expect("session attach client should connect");
    configure_connection_timeouts(&attach_stream, Some(Duration::from_secs(5)))
        .expect("session attach client should configure socket deadlines");
    let attach_initialize =
        initialize_named_session(&mut attach_stream, RequestId::Integer(3), "desktop-main");
    assert_ne!(
        attach_initialize.client_credential,
        opener_initialize.client_credential
    );
    let codec = JsonLineCodec;
    write_request(
        &codec,
        &mut attach_stream,
        JsonRpcRequest::new(
            RequestId::Integer(4),
            METHOD_DAEMON_SESSION_ATTACH,
            Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: opened.session.id.clone(),
                    session_authority: opened.session_authority.clone(),
                })
                .expect("session attach params should serialize"),
            ),
        ),
    );
    let error = read_error(&codec, &mut attach_stream);
    assert_eq!(error.id, Some(RequestId::Integer(4)));
    assert_eq!(error.error.code, ta_jsonrpc::INVALID_PARAMS_ERROR_CODE);
    assert_eq!(
        error.error.message,
        format!("session does not exist: {}", opened.session.id.as_str())
    );
}

#[test]
fn real_daemon_session_attach_rotates_session_authority() {
    let socket_name = unique_name("ta-daemon-it-session-attach-rotate");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before session.attach assertions");

    let client = daemon.client();
    let mut stream = connect_socket(&client.config().socket_address)
        .expect("session attach client should connect");
    configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
        .expect("session attach client should configure socket deadlines");
    initialize_named_session(&mut stream, RequestId::Integer(1), "desktop-main");
    let opened = open_session(
        &mut stream,
        RequestId::Integer(2),
        "Build daemon app server",
    );

    let attached = attach_session(
        &mut stream,
        RequestId::Integer(3),
        opened.session.id.clone(),
        opened.session_authority.clone(),
    );
    assert_eq!(attached.session.id, opened.session.id);
    assert_ne!(attached.session_authority, opened.session_authority);

    let recovered = attach_session(
        &mut stream,
        RequestId::Integer(4),
        opened.session.id.clone(),
        opened.session_authority.clone(),
    );
    assert_ne!(recovered.session_authority, attached.session_authority);

    let codec = JsonLineCodec;
    write_request(
        &codec,
        &mut stream,
        JsonRpcRequest::new(
            RequestId::Integer(5),
            METHOD_DAEMON_SESSION_ATTACH,
            Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: opened.session.id.clone(),
                    session_authority: attached.session_authority,
                })
                .expect("session attach params should serialize"),
            ),
        ),
    );
    let stale_attached_error = read_error(&codec, &mut stream);
    assert_eq!(stale_attached_error.id, Some(RequestId::Integer(5)));
    assert_eq!(
        stale_attached_error.error.code,
        ta_jsonrpc::INVALID_PARAMS_ERROR_CODE
    );
    assert_eq!(
        stale_attached_error.error.message,
        format!("session authority rejected: {}", opened.session.id.as_str())
    );

    write_request(
        &codec,
        &mut stream,
        JsonRpcRequest::new(
            RequestId::Integer(6),
            METHOD_DAEMON_SESSION_ATTACH,
            Some(
                serde_json::to_value(DaemonSessionAttachParams {
                    session_id: opened.session.id.clone(),
                    session_authority: opened.session_authority,
                })
                .expect("session attach params should serialize"),
            ),
        ),
    );
    let stale_error = read_error(&codec, &mut stream);
    assert_eq!(stale_error.id, Some(RequestId::Integer(6)));
    assert_eq!(
        stale_error.error.code,
        ta_jsonrpc::INVALID_PARAMS_ERROR_CODE
    );
    assert_eq!(
        stale_error.error.message,
        format!("session authority rejected: {}", opened.session.id.as_str())
    );

    let rotated = attach_session(
        &mut stream,
        RequestId::Integer(7),
        opened.session.id,
        recovered.session_authority.clone(),
    );
    assert_ne!(rotated.session_authority, recovered.session_authority);
}

#[test]
fn real_daemon_run_start_creates_session_scoped_run_over_canonical_protocol() {
    let socket_name = unique_name("ta-daemon-it-run-start");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before run.start assertions");

    let client = daemon.client();
    let mut stream =
        connect_socket(&client.config().socket_address).expect("run.start client should connect");
    configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
        .expect("run.start client should configure socket deadlines");
    initialize_named_session(&mut stream, RequestId::Integer(1), "desktop-main");
    let opened = open_session(
        &mut stream,
        RequestId::Integer(2),
        "Build daemon app server",
    );
    let subscribe = subscribe_events(
        &mut stream,
        RequestId::Integer(3),
        &[DaemonEventKind::Run, DaemonEventKind::Approval],
    );
    assert!(matches!(subscribe, DaemonSubscribeResult::Ready { .. }));
    let run = start_run(
        &mut stream,
        RequestId::Integer(4),
        opened.session.id.clone(),
        "Ship app server hard cut",
    );

    assert!(run.id.as_str().starts_with("run-"));
    assert_eq!(run.objective, "Ship app server hard cut");
    assert_eq!(run.status, ta_protocol::wire::RunStatus::WaitingForApproval);

    let waiting_event = read_event_notification(&JsonLineCodec, &mut stream);
    let waiting_envelope: DaemonEventEnvelope =
        serde_json::from_value(waiting_event.params.expect("event params should exist"))
            .expect("daemon event params should deserialize");
    assert!(matches!(
        waiting_envelope.event,
        ta_protocol::wire::DaemonEvent::Run(ta_protocol::wire::RunEvent {
            run_id,
            status,
            detail,
            ..
        })
            if run_id == run.id
                && status == ta_protocol::wire::RunStatus::WaitingForApproval
                && detail == "Waiting for approval"
    ));

    let approval_event = read_event_notification(&JsonLineCodec, &mut stream);
    let approval_envelope: DaemonEventEnvelope =
        serde_json::from_value(approval_event.params.expect("event params should exist"))
            .expect("daemon event params should deserialize");
    assert!(matches!(
        approval_envelope.event,
        ta_protocol::wire::DaemonEvent::Approval(
            ta_protocol::wire::ApprovalEvent::Requested { request }
        )
            if request.run_id == run.id
                && request.scope == ta_protocol::wire::ApprovalScope::ProcessExec
    ));

    let session = get_session(
        &mut stream,
        RequestId::Integer(5),
        opened.session.id.clone(),
    );
    assert_eq!(
        session.expect("session should exist").status,
        ta_protocol::wire::SessionStatus::Running
    );

    let runs = list_runs(
        &mut stream,
        RequestId::Integer(6),
        opened.session.id.clone(),
    );
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, run.id);
    assert_eq!(
        runs[0].status,
        ta_protocol::wire::RunStatus::WaitingForApproval
    );

    let approvals = list_approvals(
        &mut stream,
        RequestId::Integer(7),
        opened.session.id.clone(),
        Some(run.id.clone()),
        None,
    );
    assert_eq!(approvals.items.len(), 1);
    assert_eq!(approvals.items[0].run_id, run.id);
    assert_eq!(
        approvals.items[0].scope,
        ta_protocol::wire::ApprovalScope::ProcessExec
    );
    assert!(approvals.latest_cursor.is_some());

    let activity_page = activity_page(
        &mut stream,
        RequestId::Integer(8),
        opened.session.id,
        vec![DaemonEventKind::Run, DaemonEventKind::Approval],
    );
    assert_eq!(activity_page.items.len(), 2);
    assert!(matches!(
        &activity_page.items[0].event,
        ta_protocol::wire::DaemonEvent::Approval(
            ta_protocol::wire::ApprovalEvent::Requested { request }
        )
            if request.run_id == run.id
                && request.scope == ta_protocol::wire::ApprovalScope::ProcessExec
    ));
    assert!(matches!(
        &activity_page.items[1].event,
        ta_protocol::wire::DaemonEvent::Run(ta_protocol::wire::RunEvent {
            run_id,
            status,
            detail,
            ..
        })
            if *run_id == run.id
                && *status == ta_protocol::wire::RunStatus::WaitingForApproval
                && detail == "Waiting for approval"
    ));
}

#[test]
fn real_daemon_replay_run_events_replays_history_after_cursor_and_rejects_session_mismatch() {
    let socket_name = unique_name("ta-daemon-it-run-replay");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before run replay assertions");

    let client = daemon.client();
    let mut stream =
        connect_socket(&client.config().socket_address).expect("run replay client should connect");
    configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
        .expect("run replay client should configure socket deadlines");
    initialize_named_session(&mut stream, RequestId::Integer(1), "desktop-main");
    let opened = open_session(
        &mut stream,
        RequestId::Integer(2),
        "Replay native run events",
    );
    let after_open_sequence = opened
        .latest_cursor
        .as_ref()
        .expect("session open should expose durable cursor")
        .sequence;
    let run = start_run(
        &mut stream,
        RequestId::Integer(3),
        opened.session.id.clone(),
        "Replay native event history",
    );

    let replay = replay_run_events(
        &mut stream,
        RequestId::Integer(4),
        opened.session.id.clone(),
        run.id.clone(),
        Some(after_open_sequence),
    );
    assert_eq!(replay.events.len(), 2);
    assert!(
        replay
            .events
            .iter()
            .all(|event| event.seq > after_open_sequence)
    );
    assert_eq!(
        replay.latest_event_seq,
        replay.events.last().map(|event| event.seq)
    );
    assert!(matches!(
        &replay.events[0].event,
        ta_protocol::wire::PublicDaemonEvent::Run(ta_protocol::wire::RunEvent { run_id, status, .. })
            if *run_id == run.id && *status == RunStatus::WaitingForApproval
    ));
    assert!(matches!(
        &replay.events[1].event,
        ta_protocol::wire::PublicDaemonEvent::Approval(
            ta_protocol::wire::PublicApprovalEvent::Requested { request }
        ) if request.run_id == run.id
    ));

    let tail_replay = replay_run_events(
        &mut stream,
        RequestId::Integer(5),
        opened.session.id.clone(),
        run.id.clone(),
        replay.events.first().map(|event| event.seq),
    );
    assert_eq!(
        tail_replay
            .events
            .iter()
            .map(|event| event.seq)
            .collect::<Vec<_>>(),
        replay
            .events
            .iter()
            .skip(1)
            .map(|event| event.seq)
            .collect::<Vec<_>>()
    );

    let mismatch = replay_run_events_expect_invalid_params(
        &mut stream,
        RequestId::Integer(6),
        SessionId::new("session-mismatch").expect("session id"),
        run.id,
        None,
    );
    assert_eq!(mismatch.error.code, INVALID_PARAMS_ERROR_CODE);
    assert!(
        mismatch
            .error
            .message
            .contains("run does not belong to session")
    );
}
