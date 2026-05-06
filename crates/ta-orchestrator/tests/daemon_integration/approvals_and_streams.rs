use crate::support::*;

#[test]
fn real_daemon_reattach_subscribe_from_activity_cursor_only_tails_newer_events() {
    let socket_name = unique_name("ta-daemon-it-reattach-activity-cursor");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before reattach activity assertions");

    let client = daemon.client();
    let mut first_stream = connect_socket(&client.config().socket_address)
        .expect("first activity client should connect");
    configure_connection_timeouts(&first_stream, Some(Duration::from_secs(5)))
        .expect("first activity client should configure socket deadlines");
    let first_initialize =
        initialize_named_session(&mut first_stream, RequestId::Integer(1), "desktop-main");
    let opened = open_session(
        &mut first_stream,
        RequestId::Integer(2),
        "Build daemon app server",
    );
    let subscribe = subscribe_events(
        &mut first_stream,
        RequestId::Integer(3),
        &[DaemonEventKind::Run, DaemonEventKind::Approval],
    );
    assert!(matches!(subscribe, DaemonSubscribeResult::Ready { .. }));
    let run = start_run(
        &mut first_stream,
        RequestId::Integer(4),
        opened.session.id.clone(),
        "Ship app server hard cut",
    );
    let _waiting_event = read_event_notification(&JsonLineCodec, &mut first_stream);
    let approval_event = read_event_notification(&JsonLineCodec, &mut first_stream);
    let approval_envelope: DaemonEventEnvelope =
        serde_json::from_value(approval_event.params.expect("event params should exist"))
            .expect("daemon event params should deserialize");
    let approval_id = match approval_envelope.event {
        ta_protocol::wire::DaemonEvent::Approval(ta_protocol::wire::ApprovalEvent::Requested {
            request,
        }) => request.id,
        other => panic!("expected approval request event, got {other:?}"),
    };

    let _latest_activity_cursor = activity_page(
        &mut first_stream,
        RequestId::Integer(5),
        opened.session.id.clone(),
        vec![DaemonEventKind::Run, DaemonEventKind::Approval],
    )
    .latest_activity_cursor
    .expect("activity page should expose latest cursor");
    drop(first_stream);

    let mut second_stream = connect_socket(&client.config().socket_address)
        .expect("reattach activity client should connect");
    configure_connection_timeouts(&second_stream, Some(Duration::from_secs(5)))
        .expect("reattach activity client should configure socket deadlines");
    initialize_named_session_with_credential(
        &mut second_stream,
        RequestId::Integer(6),
        "desktop-main",
        Some(&first_initialize.client_credential),
    );
    let attached = attach_session(
        &mut second_stream,
        RequestId::Integer(7),
        opened.session.id.clone(),
        opened.session_authority.clone(),
    );
    let latest_cursor = attached
        .latest_cursor
        .clone()
        .expect("attach should expose current resume cursor");
    let replay = subscribe_events_after_cursor(
        &mut second_stream,
        RequestId::Integer(8),
        &[DaemonEventKind::Run, DaemonEventKind::Approval],
        Some(latest_cursor.clone()),
    );
    assert_eq!(
        replay,
        DaemonSubscribeResult::Ready {
            latest_cursor: Some(latest_cursor.clone()),
        }
    );

    let decided = decide_approval(
        &mut second_stream,
        RequestId::Integer(9),
        opened.session.id,
        approval_id.clone(),
        ApprovalDecision::Approved,
    );
    assert_eq!(decided.run.id, run.id);
    assert!(
        matches!(
            decided.run.status,
            ta_protocol::wire::RunStatus::Running | ta_protocol::wire::RunStatus::Failed
        ),
        "expected post-approval intermediate status to be Running or Failed, got {:?}",
        decided.run.status
    );

    let resolved_event = read_event_notification(&JsonLineCodec, &mut second_stream);
    let resolved_envelope: DaemonEventEnvelope =
        serde_json::from_value(resolved_event.params.expect("event params should exist"))
            .expect("daemon event params should deserialize");
    assert!(matches!(
        resolved_envelope.event,
        ta_protocol::wire::DaemonEvent::Approval(
            ta_protocol::wire::ApprovalEvent::Resolved { resolution }
        ) if resolution.approval_id == approval_id
            && resolution.run_id == run.id
            && resolution.decision == ApprovalDecision::Approved
    ));

    let terminal_envelope = read_terminal_run_event(&JsonLineCodec, &mut second_stream, &run.id);
    assert!(matches!(
        terminal_envelope.event,
        ta_protocol::wire::DaemonEvent::Run(ta_protocol::wire::RunEvent { run_id, status, .. })
            if run_id == run.id
                && status == ta_protocol::wire::RunStatus::Failed
    ));
}

#[test]
fn two_persistent_clients_can_initialize_and_subscribe_without_interference() {
    let socket_name = unique_name("ta-daemon-it-multi-client");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before multi-client attach assertions");

    let client = daemon.client();
    let mut first_stream = connect_socket(&client.config().socket_address)
        .expect("first persistent client should connect");
    configure_connection_timeouts(&first_stream, Some(Duration::from_secs(5)))
        .expect("first persistent client should configure socket deadlines");

    let mut second_stream = connect_socket(&client.config().socket_address)
        .expect("second persistent client should connect");
    configure_connection_timeouts(&second_stream, Some(Duration::from_secs(5)))
        .expect("second persistent client should configure socket deadlines");

    let first_initialize = initialize_session(&mut first_stream, RequestId::Integer(1));
    let second_initialize = initialize_session(&mut second_stream, RequestId::Integer(2));
    assert_eq!(
        first_initialize.daemon_instance_id,
        second_initialize.daemon_instance_id
    );
    assert_eq!(
        first_initialize.protocol_version,
        second_initialize.protocol_version
    );
    let first_opened = open_session(&mut first_stream, RequestId::Integer(3), "Session A");
    let second_opened = open_session(&mut second_stream, RequestId::Integer(4), "Session B");

    let first_subscribe = subscribe_run_events(&mut first_stream, RequestId::Integer(5));
    let second_subscribe = subscribe_run_events(&mut second_stream, RequestId::Integer(6));

    assert!(matches!(
        first_subscribe,
        DaemonSubscribeResult::Ready { .. }
    ));
    assert!(matches!(
        second_subscribe,
        DaemonSubscribeResult::Ready { .. }
    ));

    let run = start_run(
        &mut first_stream,
        RequestId::Integer(7),
        first_opened.session.id.clone(),
        "Run A1",
    );

    let _run_event = read_event_notification(&JsonLineCodec, &mut first_stream);

    // Windows local sockets do not support recv deadlines in this transport.
    // Give any incorrect cross-session notification a bounded chance to arrive;
    // the following response read will fail if one was queued before it.
    thread::sleep(Duration::from_millis(250));

    let second_runs = list_runs(
        &mut second_stream,
        RequestId::Integer(8),
        second_opened.session.id.clone(),
    );
    assert!(
        second_runs.is_empty(),
        "session-b reads must stay empty after session-a run.start"
    );

    let first_approvals = list_approvals(
        &mut first_stream,
        RequestId::Integer(9),
        first_opened.session.id.clone(),
        Some(run.id),
        None,
    );
    assert_eq!(first_approvals.items.len(), 1);
    assert!(first_approvals.latest_cursor.is_some());
}

#[test]
fn real_daemon_exposes_empty_app_read_models_over_canonical_protocol() {
    let socket_name = unique_name("ta-daemon-it-app-surface");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before app-surface assertions");

    let client = daemon.client();

    let mut list_stream = connect_socket(&client.config().socket_address)
        .expect("app-surface list client should connect");
    configure_connection_timeouts(&list_stream, Some(Duration::from_secs(5)))
        .expect("app-surface list client should configure socket deadlines");
    initialize_named_session(&mut list_stream, RequestId::Integer(1), "desktop-main");
    let sessions = list_sessions(&mut list_stream, RequestId::Integer(2));
    assert!(sessions.is_empty());

    let mut stream =
        connect_socket(&client.config().socket_address).expect("app-surface client should connect");
    configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
        .expect("app-surface client should configure socket deadlines");
    initialize_named_session(&mut stream, RequestId::Integer(3), "desktop-main");
    let opened = open_session(&mut stream, RequestId::Integer(4), "Empty session");

    let session = get_session(
        &mut stream,
        RequestId::Integer(5),
        opened.session.id.clone(),
    );
    assert_eq!(session, Some(opened.session.clone()));

    let runs = list_runs(
        &mut stream,
        RequestId::Integer(6),
        opened.session.id.clone(),
    );
    assert!(runs.is_empty());

    let approvals = list_approvals(
        &mut stream,
        RequestId::Integer(7),
        opened.session.id.clone(),
        None,
        None,
    );
    assert!(approvals.items.is_empty());
    assert!(approvals.latest_cursor.is_some());

    let activity_page = activity_page(
        &mut stream,
        RequestId::Integer(8),
        opened.session.id.clone(),
        vec![],
    );
    assert_eq!(activity_page.items.len(), 1);
    assert!(matches!(
        &activity_page.items[0].event,
        ta_protocol::wire::DaemonEvent::Session(ta_protocol::wire::SessionEvent {
            session_id,
            status,
        }) if *session_id == opened.session.id && *status == ta_protocol::wire::SessionStatus::Idle
    ));
    assert_eq!(activity_page.next_before, None);
    assert_eq!(
        activity_page
            .latest_activity_cursor
            .as_ref()
            .map(|cursor| cursor.sequence),
        Some(1)
    );

    let artifacts = list_artifacts(
        &mut stream,
        RequestId::Integer(9),
        opened.session.id.clone(),
    );
    assert!(artifacts.items.is_empty());
    assert!(artifacts.latest_cursor.is_some());

    let artifact = get_artifact(
        &mut stream,
        RequestId::Integer(10),
        opened.session.id.clone(),
        ta_protocol::wire::ArtifactId::new("missing-artifact")
            .expect("test artifact id should be valid"),
    );
    assert_eq!(artifact, None);

    let run = get_run(
        &mut stream,
        RequestId::Integer(9),
        opened.session.id,
        ta_protocol::wire::RunId::new("missing-run").expect("test run id should be valid"),
    );
    assert_eq!(run, None);
}

#[test]
fn real_daemon_restart_reads_store_committed_artifact_list_and_get_for_session_run() {
    let socket_name = unique_name("ta-daemon-it-restart-artifact-recovery");
    let root_dir = test_temp_dir("ta-daemon-it-restart-artifact-root");
    let artifact_id = ta_protocol::wire::ArtifactId::new("artifact-1").expect("artifact id");

    let (session_id, session_authority, client_credential, run_id) = {
        let mut daemon = ManagedDaemon::spawn_in_existing_root(&socket_name, root_dir.clone(), &[]);
        daemon
            .wait_for_status()
            .expect("real daemon should answer daemon.status before artifact recovery setup");

        let client = daemon.client();
        let mut stream = connect_socket(&client.config().socket_address)
            .expect("artifact recovery client should connect");
        configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
            .expect("artifact recovery client should configure socket deadlines");
        let initialized =
            initialize_named_session(&mut stream, RequestId::Integer(1), "desktop-main");
        let opened = open_session(&mut stream, RequestId::Integer(2), "Recover artifact");
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
            "Prepare durable artifact",
        );
        match run.status {
            RunStatus::Running => {
                let _running_event = read_event_notification(&JsonLineCodec, &mut stream);
            }
            RunStatus::WaitingForApproval => {
                let _waiting_event = read_event_notification(&JsonLineCodec, &mut stream);
                let approval_event = read_event_notification(&JsonLineCodec, &mut stream);
                let approval_envelope: DaemonEventEnvelope = serde_json::from_value(
                    approval_event.params.expect("event params should exist"),
                )
                .expect("daemon event params should deserialize");
                let approval_id = match approval_envelope.event {
                    ta_protocol::wire::DaemonEvent::Approval(
                        ta_protocol::wire::ApprovalEvent::Requested { request },
                    ) => request.id,
                    other => panic!("expected approval request event, got {other:?}"),
                };
                let decided = decide_approval(
                    &mut stream,
                    RequestId::Integer(5),
                    opened.session.id.clone(),
                    approval_id,
                    ApprovalDecision::Approved,
                );
                assert!(
                    matches!(decided.run.status, RunStatus::Running | RunStatus::Failed),
                    "expected post-approval intermediate status to be Running or Failed, got {:?}",
                    decided.run.status
                );

                let resolved_event = read_event_notification(&JsonLineCodec, &mut stream);
                let resolved_envelope: DaemonEventEnvelope = serde_json::from_value(
                    resolved_event.params.expect("event params should exist"),
                )
                .expect("daemon event params should deserialize");
                assert!(matches!(
                    resolved_envelope.event,
                    ta_protocol::wire::DaemonEvent::Approval(
                        ta_protocol::wire::ApprovalEvent::Resolved { resolution }
                    )
                        if resolution.decision == ApprovalDecision::Approved
                ));
                let terminal_envelope =
                    read_terminal_run_event(&JsonLineCodec, &mut stream, &run.id);
                assert!(matches!(
                    terminal_envelope.event,
                    ta_protocol::wire::DaemonEvent::Run(ta_protocol::wire::RunEvent {
                        status: RunStatus::Failed,
                        ..
                    })
                ));
            }
            status => panic!("unexpected start status for artifact recovery proof: {status:?}"),
        }

        daemon.shutdown();
        (
            opened.session.id,
            opened.session_authority,
            initialized.client_credential,
            run.id,
        )
    };

    force_run_running_in_existing_root_store(&root_dir, &socket_name, &run_id);
    commit_artifact_in_existing_root_store(
        &root_dir,
        &socket_name,
        ArtifactRecord {
            id: artifact_id.clone(),
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            kind: ta_protocol::wire::ArtifactKind::Patch,
            storage_path: "artifacts/run-1/patch.diff".to_string(),
        },
    );

    let mut daemon = ManagedDaemon::spawn_in_existing_root(&socket_name, root_dir.clone(), &[]);
    daemon
        .wait_for_status()
        .expect("restarted daemon should answer daemon.status");

    let client = daemon.client();
    let mut stream = connect_socket(&client.config().socket_address)
        .expect("artifact recovery reconnect client should reconnect");
    configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
        .expect("artifact recovery reconnect client should configure socket deadlines");
    initialize_named_session_with_credential(
        &mut stream,
        RequestId::Integer(4),
        "desktop-main",
        Some(&client_credential),
    );
    let attached = attach_session(
        &mut stream,
        RequestId::Integer(5),
        session_id.clone(),
        session_authority,
    );
    assert_eq!(attached.session.id, session_id);

    let artifacts = list_artifacts(&mut stream, RequestId::Integer(6), session_id.clone());
    assert_eq!(artifacts.items.len(), 1);
    assert_eq!(artifacts.items[0].id, artifact_id);
    assert_eq!(artifacts.items[0].run_id, run_id);
    assert!(artifacts.latest_cursor.is_some());

    let artifact = get_artifact(
        &mut stream,
        RequestId::Integer(7),
        session_id,
        artifact_id.clone(),
    )
    .expect("artifact should recover after restart");
    assert_eq!(artifact.id, artifact_id);
    assert_eq!(artifact.run_id, run_id);

    let _ = fs::remove_dir_all(&root_dir);
}

#[test]
fn real_daemon_restart_fails_checkpointed_running_run() {
    let socket_name = unique_name("ta-daemon-it-restart-checkpoint-resume");
    let root_dir = test_temp_dir("ta-daemon-it-restart-checkpoint-root");

    let (session_id, session_authority, client_credential, run_id) = {
        let mut daemon = ManagedDaemon::spawn_in_existing_root(&socket_name, root_dir.clone(), &[]);
        daemon
            .wait_for_status()
            .expect("real daemon should answer daemon.status before checkpoint resume setup");

        let client = daemon.client();
        let mut stream = connect_socket(&client.config().socket_address)
            .expect("checkpoint resume client should connect");
        configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
            .expect("checkpoint resume client should configure socket deadlines");
        let initialized =
            initialize_named_session(&mut stream, RequestId::Integer(1), "desktop-main");
        let opened = open_session(
            &mut stream,
            RequestId::Integer(2),
            "Resume checkpointed run",
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
            "Checkpoint before daemon restart",
        );
        let run_id = match run.status {
            RunStatus::Running => {
                let _running_event = read_event_notification(&JsonLineCodec, &mut stream);
                run.id.clone()
            }
            RunStatus::WaitingForApproval => {
                let _waiting_event = read_event_notification(&JsonLineCodec, &mut stream);
                let approval_event = read_event_notification(&JsonLineCodec, &mut stream);
                let approval_envelope: DaemonEventEnvelope = serde_json::from_value(
                    approval_event.params.expect("event params should exist"),
                )
                .expect("daemon event params should deserialize");
                let approval_id = match approval_envelope.event {
                    ta_protocol::wire::DaemonEvent::Approval(
                        ta_protocol::wire::ApprovalEvent::Requested { request },
                    ) => request.id,
                    other => panic!("expected approval request event, got {other:?}"),
                };
                let decided = decide_approval(
                    &mut stream,
                    RequestId::Integer(5),
                    opened.session.id.clone(),
                    approval_id,
                    ApprovalDecision::Approved,
                );
                assert!(
                    matches!(decided.run.status, RunStatus::Running | RunStatus::Failed),
                    "expected post-approval intermediate status to be Running or Failed, got {:?}",
                    decided.run.status
                );

                let _resolved_event = read_event_notification(&JsonLineCodec, &mut stream);
                let _terminal_event = read_terminal_run_event(&JsonLineCodec, &mut stream, &run.id);
                decided.run.id
            }
            status => panic!("unexpected start status for checkpoint resume proof: {status:?}"),
        };

        daemon.shutdown();
        (
            opened.session.id,
            opened.session_authority,
            initialized.client_credential,
            run_id,
        )
    };

    force_run_running_in_existing_root_store(&root_dir, &socket_name, &run_id);
    commit_checkpoint_in_existing_root_store(
        &root_dir,
        &socket_name,
        CheckpointRecord {
            run_id: run_id.clone(),
            revision: 1,
            artifact_path: format!("checkpoints/{}/rev-1.json", run_id.as_str()),
        },
    );

    let mut daemon = ManagedDaemon::spawn_in_existing_root(&socket_name, root_dir.clone(), &[]);
    daemon
        .wait_for_status()
        .expect("restarted daemon should answer daemon.status");

    let client = daemon.client();
    let mut stream = connect_socket(&client.config().socket_address)
        .expect("checkpoint resume reconnect client should reconnect");
    configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
        .expect("checkpoint resume reconnect client should configure socket deadlines");
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

    let run = get_run(
        &mut stream,
        RequestId::Integer(8),
        session_id.clone(),
        run_id.clone(),
    )
    .expect("checkpointed run should recover after restart");
    assert_eq!(run.summary.id, run_id);
    assert_eq!(run.summary.status, RunStatus::Failed);

    let activity = activity_page(
        &mut stream,
        RequestId::Integer(9),
        session_id,
        vec![DaemonEventKind::Run],
    );
    assert!(activity.items.iter().any(|item| {
        matches!(
            &item.event,
            ta_protocol::wire::DaemonEvent::Run(run_event)
                if run_event.run_id == run_id
                    && run_event.status == RunStatus::Failed
                    && run_event.detail == "daemon restarted while run was active"
        )
    }));

    let _ = fs::remove_dir_all(&root_dir);
}
