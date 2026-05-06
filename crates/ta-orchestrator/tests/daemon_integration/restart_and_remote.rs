use crate::support::*;

#[test]
fn real_daemon_restart_allows_approval_decide_and_clears_pending_state() {
    let socket_name = unique_name("ta-daemon-it-restart-approval-decide");
    let root_dir = test_temp_dir("ta-daemon-it-restart-approval-root");

    let (session_id, session_authority, client_credential, run_id) = {
        let mut daemon = ManagedDaemon::spawn_in_existing_root(&socket_name, root_dir.clone(), &[]);
        daemon
            .wait_for_status()
            .expect("real daemon should answer daemon.status before approval recovery setup");

        let client = daemon.client();
        let mut stream = connect_socket(&client.config().socket_address)
            .expect("approval recovery client should connect");
        configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
            .expect("approval recovery client should configure socket deadlines");
        let initialized =
            initialize_named_session(&mut stream, RequestId::Integer(1), "desktop-main");
        let opened = open_session(&mut stream, RequestId::Integer(2), "Recover approval");
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
            "Recover pending approval",
        );
        let _waiting_event = read_event_notification(&JsonLineCodec, &mut stream);
        let approval_event = read_event_notification(&JsonLineCodec, &mut stream);
        let approval_envelope: DaemonEventEnvelope =
            serde_json::from_value(approval_event.params.expect("event params should exist"))
                .expect("daemon event params should deserialize");
        let _approval_id = match approval_envelope.event {
            ta_protocol::wire::DaemonEvent::Approval(
                ta_protocol::wire::ApprovalEvent::Requested { request },
            ) => request.id,
            other => panic!("expected approval request event, got {other:?}"),
        };

        daemon.shutdown();
        (
            opened.session.id,
            opened.session_authority,
            initialized.client_credential,
            run.id,
        )
    };

    let mut daemon = ManagedDaemon::spawn_in_existing_root(&socket_name, root_dir.clone(), &[]);
    daemon
        .wait_for_status()
        .expect("restarted daemon should answer daemon.status");

    let client = daemon.client();
    let mut stream = connect_socket(&client.config().socket_address)
        .expect("approval recovery reconnect client should reconnect");
    configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
        .expect("approval recovery reconnect client should configure socket deadlines");
    initialize_named_session_with_credential(
        &mut stream,
        RequestId::Integer(5),
        "desktop-main",
        Some(&client_credential),
    );
    let attached = attach_session(
        &mut stream,
        RequestId::Integer(6),
        session_id.clone(),
        session_authority,
    );
    assert_eq!(attached.session.id, session_id);
    let subscribe = subscribe_events(
        &mut stream,
        RequestId::Integer(7),
        &[DaemonEventKind::Run, DaemonEventKind::Approval],
    );
    assert!(matches!(subscribe, DaemonSubscribeResult::Ready { .. }));

    let approvals = list_approvals(
        &mut stream,
        RequestId::Integer(8),
        session_id.clone(),
        Some(run_id.clone()),
        None,
    );
    assert!(approvals.items.is_empty());
    assert!(approvals.latest_cursor.is_some());

    let run = get_run(
        &mut stream,
        RequestId::Integer(9),
        session_id,
        run_id.clone(),
    )
    .expect("run should recover after approval decide");
    assert_eq!(run.summary.id, run_id);
    assert_eq!(run.summary.status, RunStatus::Failed);

    let _ = fs::remove_dir_all(&root_dir);
}

#[test]
fn real_daemon_restart_subscribe_from_persisted_activity_cursor_only_tails_newer_events() {
    let socket_name = unique_name("ta-daemon-it-restart-subscribe-cursor");
    let root_dir = test_temp_dir("ta-daemon-it-restart-subscribe-root");

    let (
        session_id,
        session_authority,
        client_credential,
        _run_id,
        _approval_id,
        stale_cursor,
        stale_sequence,
    ) = {
        let mut daemon = ManagedDaemon::spawn_in_existing_root(&socket_name, root_dir.clone(), &[]);
        daemon
            .wait_for_status()
            .expect("real daemon should answer daemon.status before restart subscribe setup");

        let client = daemon.client();
        let mut stream = connect_socket(&client.config().socket_address)
            .expect("restart subscribe client should connect");
        configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
            .expect("restart subscribe client should configure socket deadlines");
        let initialized =
            initialize_named_session(&mut stream, RequestId::Integer(1), "desktop-main");
        let opened = open_session(
            &mut stream,
            RequestId::Integer(2),
            "Recover subscribe cursor",
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
            "Resume after restart",
        );
        let _waiting_event = read_event_notification(&JsonLineCodec, &mut stream);
        let approval_event = read_event_notification(&JsonLineCodec, &mut stream);
        let approval_envelope: DaemonEventEnvelope =
            serde_json::from_value(approval_event.params.expect("event params should exist"))
                .expect("daemon event params should deserialize");
        let approval_id = match approval_envelope.event {
            ta_protocol::wire::DaemonEvent::Approval(
                ta_protocol::wire::ApprovalEvent::Requested { request },
            ) => request.id,
            other => panic!("expected approval request event, got {other:?}"),
        };
        let stale_sequence = activity_page(
            &mut stream,
            RequestId::Integer(5),
            opened.session.id.clone(),
            vec![DaemonEventKind::Run, DaemonEventKind::Approval],
        )
        .latest_activity_cursor
        .map(|cursor| cursor.sequence)
        .expect("activity page should expose latest cursor");
        let stale_cursor = DaemonEventCursor {
            daemon_instance_id: initialized.daemon_instance_id,
            session_id: opened.session.id.clone(),
            sequence: stale_sequence,
        };

        daemon.shutdown();
        (
            opened.session.id,
            opened.session_authority,
            initialized.client_credential,
            run.id,
            approval_id,
            stale_cursor,
            stale_sequence,
        )
    };

    let mut daemon = ManagedDaemon::spawn_in_existing_root(&socket_name, root_dir.clone(), &[]);
    daemon
        .wait_for_status()
        .expect("restarted daemon should answer daemon.status");

    let client = daemon.client();
    let mut stream = connect_socket(&client.config().socket_address)
        .expect("restart subscribe reconnect client should reconnect");
    configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
        .expect("restart subscribe reconnect client should configure socket deadlines");
    initialize_named_session_with_credential(
        &mut stream,
        RequestId::Integer(6),
        "desktop-main",
        Some(&client_credential),
    );
    let attached = attach_session(
        &mut stream,
        RequestId::Integer(7),
        session_id.clone(),
        session_authority,
    );
    assert_eq!(attached.session.id, session_id);
    let latest_cursor = attached
        .latest_cursor
        .clone()
        .expect("attach should expose current resume cursor");
    assert_eq!(latest_cursor.session_id, session_id);
    assert_ne!(
        latest_cursor.daemon_instance_id,
        stale_cursor.daemon_instance_id
    );
    assert_eq!(latest_cursor.sequence, stale_sequence + 3);

    let replay = subscribe_events_after_cursor(
        &mut stream,
        RequestId::Integer(8),
        &[DaemonEventKind::Run, DaemonEventKind::Approval],
        Some(stale_cursor.clone()),
    );
    assert_eq!(
        replay,
        DaemonSubscribeResult::HistoryGap {
            latest_cursor: Some(latest_cursor.clone()),
        }
    );

    let _ = fs::remove_dir_all(&root_dir);
}

#[test]
fn real_daemon_remote_websocket_restart_subscribe_from_persisted_cursor_only_tails_newer_events() {
    let socket_name = unique_name("ta-daemon-it-remote-ws-restart-subscribe");
    let root_dir = test_temp_dir("ta-daemon-it-remote-ws-restart-root");
    let remote_bind = reserve_tcp_address();
    let auth_token = "0123456789abcdef0123456789abcdef";
    let daemon_env = [
        (DAEMON_REMOTE_WS_ENABLED_ENV_VAR, "1"),
        (DAEMON_REMOTE_WS_BIND_ENV_VAR, remote_bind.as_str()),
        (DAEMON_REMOTE_WS_AUTH_TOKEN_ENV_VAR, auth_token),
    ];

    let (
        session_id,
        session_authority,
        client_credential,
        _run_id,
        _approval_id,
        stale_cursor,
        stale_sequence,
    ) = {
        let mut daemon =
            ManagedDaemon::spawn_in_existing_root(&socket_name, root_dir.clone(), &daemon_env);
        daemon.wait_for_status().expect(
            "real daemon should answer daemon.status before remote websocket restart setup",
        );

        let client = daemon.client();
        let mut stream = connect_socket(&client.config().socket_address)
            .expect("remote websocket restart setup client should connect");
        configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
            .expect("remote websocket restart setup client should configure socket deadlines");
        let initialized =
            initialize_named_session(&mut stream, RequestId::Integer(1), "desktop-main");
        let opened = open_session(
            &mut stream,
            RequestId::Integer(2),
            "Recover remote websocket subscribe cursor",
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
            "Resume remote websocket after restart",
        );
        let _waiting_event = read_event_notification(&JsonLineCodec, &mut stream);
        let approval_event = read_event_notification(&JsonLineCodec, &mut stream);
        let approval_envelope: DaemonEventEnvelope =
            serde_json::from_value(approval_event.params.expect("event params should exist"))
                .expect("daemon event params should deserialize");
        let approval_id = match approval_envelope.event {
            ta_protocol::wire::DaemonEvent::Approval(
                ta_protocol::wire::ApprovalEvent::Requested { request },
            ) => request.id,
            other => panic!("expected approval request event, got {other:?}"),
        };
        let stale_sequence = activity_page(
            &mut stream,
            RequestId::Integer(5),
            opened.session.id.clone(),
            vec![DaemonEventKind::Run, DaemonEventKind::Approval],
        )
        .latest_activity_cursor
        .map(|cursor| cursor.sequence)
        .expect("activity page should expose latest cursor");
        let stale_cursor = DaemonEventCursor {
            daemon_instance_id: initialized.daemon_instance_id,
            session_id: opened.session.id.clone(),
            sequence: stale_sequence,
        };

        daemon.shutdown();
        (
            opened.session.id,
            opened.session_authority,
            initialized.client_credential,
            run.id,
            approval_id,
            stale_cursor,
            stale_sequence,
        )
    };

    let mut daemon =
        ManagedDaemon::spawn_in_existing_root(&socket_name, root_dir.clone(), &daemon_env);
    daemon
        .wait_for_status()
        .expect("restarted daemon should answer daemon.status");

    let mut socket = connect_remote_websocket(&remote_bind, auth_token)
        .expect("remote websocket should reconnect");
    initialize_remote_client_with_credential(&mut socket, 6, Some(&client_credential));
    let attached = attach_remote_session(&mut socket, 7, session_id.clone(), session_authority);
    assert_eq!(attached.session.id, session_id);
    let latest_cursor = attached
        .latest_cursor
        .clone()
        .expect("attach should expose current resume cursor");
    assert_eq!(latest_cursor.session_id, session_id);
    assert_ne!(
        latest_cursor.daemon_instance_id,
        stale_cursor.daemon_instance_id
    );
    assert_eq!(latest_cursor.sequence, stale_sequence + 3);

    let replay = subscribe_remote_events_after_cursor(
        &mut socket,
        8,
        &[DaemonEventKind::Run, DaemonEventKind::Approval],
        Some(stale_cursor.clone()),
    );
    assert_eq!(
        replay,
        DaemonSubscribeResult::HistoryGap {
            latest_cursor: Some(latest_cursor.clone()),
        }
    );

    let _ = fs::remove_dir_all(&root_dir);
}
