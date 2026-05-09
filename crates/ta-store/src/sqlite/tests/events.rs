use super::*;

#[test]
fn read_run_events_uses_session_cursor_and_preserves_run_limit() {
    let path = test_db_path("run-events-session-cursor");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-replay").expect("session id");
    let foreign_session_id = SessionId::new("session-foreign").expect("session id");
    let run_id = RunId::new("run-replay").expect("run id");
    let other_run_id = RunId::new("run-other").expect("run id");

    for session in [&session_id, &foreign_session_id] {
        store
            .save_session(SessionProjection {
                id: session.clone(),
                owner_client_name: "sqlite-tests".to_string(),
                owner_principal_id: "principal-test-owner".to_string(),
                current_session_authority_hash: "session-authority-hash".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Replay".to_string(),
                status: SessionStatus::Running,
                workspace_id: crate::default_test_workspace_id(),
            })
            .expect("session should persist");
    }

    let mut append_run_event = |sequence: u64, session_id: &SessionId, run_id: &RunId| {
        store
            .append_event(EventRecord {
                sequence,
                session_id: session_id.clone(),
                occurred_at_ms: sequence * 10,
                payload: DaemonEvent::Run(ta_protocol::wire::RunEvent {
                    run_id: run_id.clone(),
                    status: RunStatus::Running,
                    detail: format!("event {sequence}"),
                    output_contract: None,
                    recipe_id: None,
                    result: None,
                }),
            })
            .expect("event should seed");
    };

    for sequence in 1..=300 {
        append_run_event(sequence, &session_id, &other_run_id);
    }
    for sequence in 301..=303 {
        append_run_event(sequence, &session_id, &run_id);
    }
    for sequence in 304..=600 {
        append_run_event(sequence, &session_id, &other_run_id);
    }
    for sequence in 601..=700 {
        append_run_event(sequence, &foreign_session_id, &run_id);
    }

    let first_page = ok(store.read_run_events(&RunEventRangeQuery {
        session_id: session_id.clone(),
        run_id: run_id.clone(),
        after_sequence: Some(0),
        limit: 2,
    }));
    assert_eq!(
        first_page
            .records
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![301, 302]
    );
    assert_eq!(first_page.latest_sequence, Some(303));

    let second_page = ok(store.read_run_events(&RunEventRangeQuery {
        session_id,
        run_id,
        after_sequence: Some(302),
        limit: 2,
    }));
    assert_eq!(
        second_page
            .records
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![303]
    );
    assert_eq!(second_page.latest_sequence, Some(303));

    let _ = std::fs::remove_file(path);
}

#[test]
fn approval_lookup_distinguishes_pending_resolved_and_not_found_after_reopen() {
    let path = test_db_path("approval-lookup");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    let pending_id = ApprovalId::new("approval-pending").expect("approval id");
    let resolved_id = ApprovalId::new("approval-resolved").expect("approval id");
    {
        let mut store = SqliteStore::open(&path).expect("store should open");
        store
            .save_session(SessionProjection {
                id: session_id.clone(),
                owner_client_name: "sqlite-tests".to_string(),
                owner_principal_id: "principal-test-owner".to_string(),
                current_session_authority_hash: "session-authority-hash".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Approval lookup".to_string(),
                status: SessionStatus::Running,
                workspace_id: crate::default_test_workspace_id(),
            })
            .expect("session");
        store
            .save_run(RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Lookup approvals".to_string(),
                status: RunStatus::WaitingForApproval,
                source: RunSource::default(),
                harness: RunHarnessKind::Unknown,
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: None,
                claimed_files: Vec::new(),
                conflict_summary: None,
            })
            .expect("run");
        store
            .append_event(EventRecord {
                sequence: 1,
                session_id: session_id.clone(),
                occurred_at_ms: 10,
                payload: DaemonEvent::Approval(ApprovalEvent::Requested {
                    request: ApprovalRequest::new(
                        pending_id.clone(),
                        run_id.clone(),
                        ApprovalScope::FileWrite,
                        100,
                        200,
                        ta_protocol::wire::ApprovalTarget::FileWrite {
                            paths: vec!["src/lib.rs".to_string()],
                        },
                        "pending approval",
                    )
                    .expect("approval request"),
                }),
            })
            .expect("pending request");
        store
            .append_event(EventRecord {
                sequence: 2,
                session_id: session_id.clone(),
                occurred_at_ms: 20,
                payload: DaemonEvent::Approval(ApprovalEvent::Requested {
                    request: ApprovalRequest::new(
                        resolved_id.clone(),
                        run_id.clone(),
                        ApprovalScope::ProcessExec,
                        100,
                        200,
                        ta_protocol::wire::ApprovalTarget::ProcessExec { command: None },
                        "resolved approval",
                    )
                    .expect("approval request"),
                }),
            })
            .expect("resolved request");
        store
            .append_event(EventRecord {
                sequence: 3,
                session_id: session_id.clone(),
                occurred_at_ms: 30,
                payload: DaemonEvent::Approval(ApprovalEvent::Resolved {
                    resolution: ta_protocol::wire::ApprovalResolution::new(
                        resolved_id.clone(),
                        run_id,
                        ApprovalDecision::Approved,
                        ta_protocol::wire::ApprovalResolutionReason::User,
                        ta_protocol::wire::ApprovalActor::new("principal-sqlite-tests")
                            .expect("approval actor"),
                        None,
                    ),
                }),
            })
            .expect("resolved event");
    }

    let store = SqliteStore::open(&path).expect("store should reopen");
    assert!(matches!(
        ok(store.approval_lookup(&session_id, &pending_id)),
        crate::SessionApprovalLookup::Pending(approval) if approval.id == pending_id
    ));
    assert_eq!(
        ok(store.approval_lookup(&session_id, &resolved_id)),
        crate::SessionApprovalLookup::Resolved
    );
    assert_eq!(
        ok(store.approval_lookup(
            &session_id,
            &ApprovalId::new("approval-missing").expect("approval id")
        )),
        crate::SessionApprovalLookup::NotFound
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn session_event_page_returns_decode_record_for_corrupt_event_payload() {
    let path = test_db_path("event-decode-corruption");
    let session_id = SessionId::new("session-1").expect("session id");
    {
        let mut store = SqliteStore::open(&path).expect("store should open");
        crate::WorkspaceRepository::upsert_workspace(&mut store, crate::default_test_workspace())
            .expect("seed workspace");

        store
            .commit_session_open(CommitSessionOpen {
                session: SessionProjection {
                    id: session_id.clone(),
                    owner_client_name: "sqlite-tests".to_string(),
                    owner_principal_id: "principal-test-owner".to_string(),
                    current_session_authority_hash: "session-authority-hash".to_string(),
                    current_session_authority_generation: 0,
                    recovery_session_authority_hash: None,
                    recovery_session_authority_generation: None,
                    title: "Persisted".to_string(),
                    status: SessionStatus::Idle,
                    workspace_id: crate::default_test_workspace_id(),
                },
                occurred_at_ms: 20,
            })
            .expect("session");
    }

    let conn = Connection::open(&path).expect("sqlite should reopen directly");
    conn.execute(
        "UPDATE events SET payload_json = ? WHERE session_id = ?",
        params!["{", session_id.as_str()],
    )
    .expect("corrupt event json");
    drop(conn);

    let store = SqliteStore::open(&path).expect("store should reopen");
    let error = store
        .session_event_page(&crate::SessionEventPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
            kinds: vec![],
        })
        .expect_err("event page read must fail on corrupt json");
    assert_eq!(
        error,
        StoreError::DecodeRecord {
            entity: "daemon_event",
            source: serde_json::Error::io(std::io::Error::other("ignored")),
        }
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn events_read_returns_decode_record_for_corrupt_event_payload_after_reopen() {
    let path = test_db_path("events-decode-corruption");
    let session_id = SessionId::new("session-1").expect("session id");
    {
        let mut store = SqliteStore::open(&path).expect("store should open");
        crate::WorkspaceRepository::upsert_workspace(&mut store, crate::default_test_workspace())
            .expect("seed workspace");

        store
            .commit_session_open(CommitSessionOpen {
                session: SessionProjection {
                    id: session_id.clone(),
                    owner_client_name: "sqlite-tests".to_string(),
                    owner_principal_id: "principal-test-owner".to_string(),
                    current_session_authority_hash: "session-authority-hash".to_string(),
                    current_session_authority_generation: 0,
                    recovery_session_authority_hash: None,
                    recovery_session_authority_generation: None,
                    title: "Persisted".to_string(),
                    status: SessionStatus::Idle,
                    workspace_id: crate::default_test_workspace_id(),
                },
                occurred_at_ms: 20,
            })
            .expect("session");
    }

    let conn = Connection::open(&path).expect("sqlite should reopen directly");
    conn.execute(
        "UPDATE events SET payload_json = ? WHERE sequence = 1",
        params!["{"],
    )
    .expect("corrupt event json");
    drop(conn);

    let store = SqliteStore::open(&path).expect("store should reopen");
    let error = store
        .events()
        .expect_err("event log read must fail on corrupt json");
    assert_eq!(
        error,
        StoreError::DecodeRecord {
            entity: "daemon_event",
            source: serde_json::Error::io(std::io::Error::other("ignored")),
        }
    );
    let _ = std::fs::remove_file(path);
}
