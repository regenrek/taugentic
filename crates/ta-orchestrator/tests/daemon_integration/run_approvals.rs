use crate::support::*;

#[test]
fn real_daemon_approval_decide_resolves_pending_request_over_canonical_protocol() {
    let socket_name = unique_name("ta-daemon-it-approval-decide");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before approval.decide assertions");

    let client = daemon.client();
    let mut stream = connect_socket(&client.config().socket_address)
        .expect("approval.decide client should connect");
    configure_connection_timeouts(&stream, Some(Duration::from_secs(5)))
        .expect("approval.decide client should configure socket deadlines");
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
    let _waiting_event = read_event_notification(&JsonLineCodec, &mut stream);
    let approval_event = read_event_notification(&JsonLineCodec, &mut stream);
    let approval_envelope: PublicDaemonEventEnvelope =
        serde_json::from_value(approval_event.params.expect("event params should exist"))
            .expect("daemon event params should deserialize");
    let approval_id = match approval_envelope.event {
        PublicDaemonEvent::Approval(PublicApprovalEvent::Requested { request }) => request.id,
        other => panic!("expected approval request event, got {other:?}"),
    };

    let decided = decide_approval(
        &mut stream,
        RequestId::Integer(5),
        opened.session.id.clone(),
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

    let resolved_event = read_event_notification(&JsonLineCodec, &mut stream);
    let resolved_envelope: PublicDaemonEventEnvelope =
        serde_json::from_value(resolved_event.params.expect("event params should exist"))
            .expect("daemon event params should deserialize");
    assert!(matches!(
        resolved_envelope.event,
        PublicDaemonEvent::Approval(PublicApprovalEvent::Resolved { resolution })
            if resolution.approval_id == approval_id
            && resolution.run_id == run.id
            && resolution.decision == ApprovalDecision::Approved
    ));

    let terminal_envelope = read_public_terminal_run_event(&JsonLineCodec, &mut stream, &run.id);
    assert!(matches!(
        terminal_envelope.event,
        PublicDaemonEvent::Run(ta_protocol::wire::RunEvent::Status(event))
            if event.run_id() == &run.id
                && event.status() == ta_protocol::wire::RunStatus::Failed
    ));

    let approvals = list_approvals(
        &mut stream,
        RequestId::Integer(6),
        opened.session.id.clone(),
        Some(run.id.clone()),
        None,
    );
    assert!(approvals.items.is_empty());
    assert!(approvals.latest_cursor.is_some());

    let selected_run = get_run(
        &mut stream,
        RequestId::Integer(7),
        opened.session.id.clone(),
        run.id.clone(),
    );
    assert_eq!(
        selected_run.expect("run should exist").summary.status,
        ta_protocol::wire::RunStatus::Failed
    );

    let activity_page = public_activity_page(
        &mut stream,
        RequestId::Integer(8),
        opened.session.id,
        vec![DaemonEventKind::Run, DaemonEventKind::Approval],
    );
    assert!(matches!(
        &activity_page.items[0].event,
        PublicDaemonEvent::Run(ta_protocol::wire::RunEvent::Status(event))
            if event.run_id() == &run.id
                && event.status() == ta_protocol::wire::RunStatus::Failed
    ));
    assert!(activity_page.items.iter().any(|item| {
        matches!(
            &item.event,
            PublicDaemonEvent::Approval(PublicApprovalEvent::Resolved { resolution })
                if resolution.approval_id == approval_id
                    && resolution.decision == ApprovalDecision::Approved
        )
    }));
}

#[test]
fn real_daemon_approval_decide_rejects_foreign_attached_session_without_mutation() {
    let socket_name = unique_name("ta-daemon-it-approval-decide-foreign");
    let mut daemon = ManagedDaemon::spawn(&socket_name);
    daemon
        .wait_for_status()
        .expect("real daemon should answer daemon.status before approval.decide denial assertions");

    let client = daemon.client();
    let mut owner_stream = connect_socket(&client.config().socket_address)
        .expect("approval owner client should connect");
    configure_connection_timeouts(&owner_stream, Some(Duration::from_secs(5)))
        .expect("approval owner client should configure socket deadlines");
    initialize_named_session(&mut owner_stream, RequestId::Integer(1), "desktop-main");
    let owner_session = open_session(
        &mut owner_stream,
        RequestId::Integer(2),
        "Build daemon app server",
    );
    let owner_subscribe = subscribe_events(
        &mut owner_stream,
        RequestId::Integer(3),
        &[DaemonEventKind::Run, DaemonEventKind::Approval],
    );
    assert!(matches!(
        owner_subscribe,
        DaemonSubscribeResult::Ready { .. }
    ));
    let owner_run = start_run(
        &mut owner_stream,
        RequestId::Integer(4),
        owner_session.session.id.clone(),
        "Ship app server hard cut",
    );
    let _waiting_event = read_event_notification(&JsonLineCodec, &mut owner_stream);
    let approval_event = read_event_notification(&JsonLineCodec, &mut owner_stream);
    let approval_envelope: PublicDaemonEventEnvelope =
        serde_json::from_value(approval_event.params.expect("event params should exist"))
            .expect("daemon event params should deserialize");
    let approval_id = match approval_envelope.event {
        PublicDaemonEvent::Approval(PublicApprovalEvent::Requested { request }) => request.id,
        other => panic!("expected approval request event, got {other:?}"),
    };

    let mut foreign_stream = connect_socket(&client.config().socket_address)
        .expect("foreign approval client should connect");
    configure_connection_timeouts(&foreign_stream, Some(Duration::from_secs(5)))
        .expect("foreign approval client should configure socket deadlines");
    initialize_named_session(&mut foreign_stream, RequestId::Integer(5), "desktop-main");
    let foreign_session = open_session(
        &mut foreign_stream,
        RequestId::Integer(6),
        "Foreign daemon app server",
    );
    let codec = JsonLineCodec;
    write_request(
        &codec,
        &mut foreign_stream,
        JsonRpcRequest::new(
            RequestId::Integer(7),
            METHOD_DAEMON_APPROVAL_DECIDE,
            Some(
                serde_json::to_value(DaemonApprovalDecideParams {
                    approval_id: approval_id.clone(),
                    decision: ApprovalDecision::Approved,
                    commentary: None,
                })
                .expect("approval decide params should serialize"),
            ),
        ),
    );
    let error = read_error(&codec, &mut foreign_stream);
    assert_eq!(error.id, Some(RequestId::Integer(7)));
    assert_eq!(error.error.code, ta_jsonrpc::INVALID_PARAMS_ERROR_CODE);
    assert_eq!(error.error.message, "approval is not pending");

    let approvals = list_approvals(
        &mut owner_stream,
        RequestId::Integer(8),
        owner_session.session.id.clone(),
        Some(owner_run.id.clone()),
        Some(approval_id.clone()),
    );
    assert_eq!(approvals.items.len(), 1);
    assert_eq!(approvals.items[0].id, approval_id);
    assert!(approvals.latest_cursor.is_some());

    let selected_run = get_run(
        &mut owner_stream,
        RequestId::Integer(9),
        owner_session.session.id.clone(),
        owner_run.id.clone(),
    );
    assert_eq!(
        selected_run.expect("run should still exist").summary.status,
        ta_protocol::wire::RunStatus::WaitingForApproval
    );

    let activity_page = public_activity_page(
        &mut owner_stream,
        RequestId::Integer(10),
        owner_session.session.id.clone(),
        vec![DaemonEventKind::Run, DaemonEventKind::Approval],
    );
    assert_eq!(activity_page.items.len(), 2);
    assert!(activity_page.items.iter().all(|item| {
        !matches!(
            &item.event,
            PublicDaemonEvent::Approval(PublicApprovalEvent::Resolved { .. })
        )
    }));
    assert!(activity_page.items.iter().all(|item| {
        !matches!(
            &item.event,
            PublicDaemonEvent::Run(ta_protocol::wire::RunEvent::Status(event))
                if event.run_id() == &owner_run.id
                    && event.status() == ta_protocol::wire::RunStatus::Running
        )
    }));

    let foreign_attached = get_session(
        &mut foreign_stream,
        RequestId::Integer(11),
        foreign_session.session.id.clone(),
    );
    assert_eq!(
        foreign_attached
            .expect("foreign session should still exist")
            .id,
        foreign_session.session.id
    );
}
