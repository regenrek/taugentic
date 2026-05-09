use super::*;

#[test]
fn principal_lookup_returns_saved_projection() {
    let mut store = InMemoryStore::current();
    let principal = PrincipalProjection {
        id: "principal-1".to_string(),
        client_name: "memory-tests".to_string(),
        credential_hash: "credential-hash-1".to_string(),
    };

    PrincipalRepository::save_principal(&mut store, principal.clone())
        .expect("principal projection");

    assert_eq!(
        some(store.principal_by_credential_hash("credential-hash-1")),
        principal
    );
}

#[test]
fn rotate_session_authority_consumes_recovery_slot_once() {
    let mut store = InMemoryStore::current();
    let session_id = SessionId::new("session-1").expect("session id");

    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "memory-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "authority-a0".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Selected".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session should persist");

    let rotated_once = store
        .rotate_session_authority(
            &session_id,
            "principal-test-owner",
            "authority-a0",
            "authority-a1",
        )
        .expect("rotation should read")
        .expect("current authority should rotate");
    assert_eq!(rotated_once.current_session_authority_hash, "authority-a1");
    assert_eq!(rotated_once.current_session_authority_generation, 1);
    assert_eq!(
        rotated_once.recovery_session_authority_hash.as_deref(),
        Some("authority-a0")
    );
    assert_eq!(rotated_once.recovery_session_authority_generation, Some(0));

    let recovered = store
        .rotate_session_authority(
            &session_id,
            "principal-test-owner",
            "authority-a0",
            "authority-a2",
        )
        .expect("rotation should read")
        .expect("recovery authority should recover once");
    assert_eq!(recovered.current_session_authority_hash, "authority-a2");
    assert_eq!(recovered.current_session_authority_generation, 2);
    assert_eq!(recovered.recovery_session_authority_hash, None);
    assert_eq!(recovered.recovery_session_authority_generation, None);

    assert!(
        store
            .rotate_session_authority(
                &session_id,
                "principal-test-owner",
                "authority-a0",
                "authority-a3",
            )
            .expect("rotation should read")
            .is_none()
    );
    assert!(
        store
            .rotate_session_authority(
                &session_id,
                "principal-test-owner",
                "authority-a1",
                "authority-a3",
            )
            .expect("rotation should read")
            .is_none()
    );
    let rotated_again = store
        .rotate_session_authority(
            &session_id,
            "principal-test-owner",
            "authority-a2",
            "authority-a3",
        )
        .expect("rotation should read")
        .expect("current authority should rotate again");
    assert_eq!(rotated_again.current_session_authority_hash, "authority-a3");
    assert_eq!(rotated_again.current_session_authority_generation, 3);
    assert_eq!(
        rotated_again.recovery_session_authority_hash.as_deref(),
        Some("authority-a2")
    );
    assert_eq!(rotated_again.recovery_session_authority_generation, Some(2));
}

#[test]
fn persists_runtime_facing_records_through_repository_traits() {
    let session_id = SessionId::new("session-1").expect("session id");
    let other_session_id = SessionId::new("session-2").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    let artifact_id = ArtifactId::new("artifact-1").expect("artifact id");
    let approval_id = ApprovalId::new("approval-1").expect("approval id");
    let approval = ApprovalRequest::new(
        approval_id,
        run_id.clone(),
        ApprovalScope::FileWrite,
        100,
        200,
        ta_protocol::wire::ApprovalTarget::FileWrite {
            paths: vec!["src/lib.rs".to_string()],
        },
        "workspace write",
    )
    .expect("approval request");
    let mut store = InMemoryStore::current();

    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "memory-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Build Taugentic".to_string(),
            status: SessionStatus::Running,
            workspace_id: crate::default_test_workspace_id(),
        })
        .expect("session projection");
    store
        .save_run(RunProjection {
            id: run_id.clone(),
            session_id: session_id.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Ship persistence boundary".to_string(),
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
        .expect("run projection");
    store
        .append_event(EventRecord {
            sequence: 1,
            session_id: session_id.clone(),
            occurred_at_ms: 42,
            payload: DaemonEvent::Session(SessionEvent {
                session_id: session_id.clone(),
                status: SessionStatus::Running,
            }),
        })
        .expect("session event");
    store
        .append_event(EventRecord {
            sequence: 2,
            session_id: session_id.clone(),
            occurred_at_ms: 43,
            payload: DaemonEvent::Approval(ApprovalEvent::Requested { request: approval }),
        })
        .expect("approval event");
    store
        .append_event(EventRecord {
            sequence: 3,
            session_id: session_id.clone(),
            occurred_at_ms: 44,
            payload: DaemonEvent::Run(RunEvent {
                run_id: run_id.clone(),
                status: RunStatus::WaitingForApproval,
                detail: "waiting".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            }),
        })
        .expect("run event");
    store
        .append_event(EventRecord {
            sequence: 4,
            session_id: other_session_id,
            occurred_at_ms: 45,
            payload: DaemonEvent::Session(SessionEvent {
                session_id: SessionId::new("session-2").expect("session id"),
                status: SessionStatus::Idle,
            }),
        })
        .expect("other session event");
    store
        .commit_checkpoint_persist(CommitCheckpointPersist {
            checkpoint: CheckpointRecord {
                run_id: run_id.clone(),
                revision: 1,
                artifact_path: "checkpoints/run-1/rev-1.json".to_string(),
            },
            occurred_at_ms: 46,
        })
        .expect("checkpoint");
    store
        .save_artifact(ArtifactRecord {
            id: artifact_id.clone(),
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/run-1/patch.diff".to_string(),
        })
        .expect("artifact");

    let persistence: &dyn PersistenceStore = &store;

    assert_eq!(
        some(persistence.session(&session_id)).title,
        "Build Taugentic"
    );
    assert_eq!(
        some(persistence.run(&run_id)).status,
        RunStatus::WaitingForApproval
    );
    assert_eq!(ok(persistence.events()).len(), 4);
    assert_eq!(ok(persistence.events_for_session(&session_id)).len(), 3);
    assert_eq!(ok(persistence.checkpoints_for_run(&run_id)).len(), 1);
    assert_eq!(
        some(persistence.artifact(&artifact_id)).storage_path,
        "artifacts/run-1/patch.diff"
    );
    assert_eq!(ok(persistence.artifacts_for_run(&run_id)).len(), 1);
}

#[test]
fn durable_activity_and_approval_reads_are_session_scoped_and_ordered() {
    let session_a = SessionId::new("session-a").expect("session id");
    let session_b = SessionId::new("session-b").expect("session id");
    let run_a = RunId::new("run-a").expect("run id");
    let run_b = RunId::new("run-b").expect("run id");
    let mut store = InMemoryStore::current();

    store
        .append_event(EventRecord {
            sequence: 1,
            session_id: session_a.clone(),
            occurred_at_ms: 10,
            payload: DaemonEvent::Run(RunEvent {
                run_id: run_a.clone(),
                status: RunStatus::Running,
                detail: "running-a".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            }),
        })
        .expect("session a run event");
    store
        .append_event(EventRecord {
            sequence: 2,
            session_id: session_b.clone(),
            occurred_at_ms: 20,
            payload: DaemonEvent::Approval(ApprovalEvent::Requested {
                request: ApprovalRequest::new(
                    ApprovalId::new("approval-b").expect("approval id"),
                    run_b,
                    ApprovalScope::ProcessExec,
                    100,
                    200,
                    ta_protocol::wire::ApprovalTarget::ProcessExec { command: None },
                    "other session",
                )
                .expect("approval request"),
            }),
        })
        .expect("session b approval event");
    store
        .append_event(EventRecord {
            sequence: 3,
            session_id: session_a.clone(),
            occurred_at_ms: 30,
            payload: DaemonEvent::Approval(ApprovalEvent::Requested {
                request: ApprovalRequest::new(
                    ApprovalId::new("approval-a").expect("approval id"),
                    run_a.clone(),
                    ApprovalScope::FileWrite,
                    100,
                    200,
                    ta_protocol::wire::ApprovalTarget::FileWrite {
                        paths: vec!["src/lib.rs".to_string()],
                    },
                    "latest approval",
                )
                .expect("approval request"),
            }),
        })
        .expect("session a approval event");
    store
        .append_event(EventRecord {
            sequence: 4,
            session_id: session_a.clone(),
            occurred_at_ms: 40,
            payload: DaemonEvent::Session(SessionEvent {
                session_id: session_a.clone(),
                status: SessionStatus::Running,
            }),
        })
        .expect("session a status event");

    let approvals = ok(store.approvals_for_session(&SessionApprovalQuery {
        session_id: session_a.clone(),
        run_id: Some(run_a),
        approval_id: None,
    }));
    let page = ok(store.session_event_page(&SessionEventPageQuery {
        session_id: session_a.clone(),
        before_sequence: None,
        limit: 2,
        kinds: vec![
            ta_protocol::wire::DaemonEventKind::Approval,
            ta_protocol::wire::DaemonEventKind::Run,
        ],
    }));

    assert_eq!(approvals.len(), 1);
    assert_eq!(approvals[0].reason, "latest approval");
    assert_eq!(
        page.records
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![3, 1]
    );
    assert_eq!(page.latest_sequence, Some(3));
    assert_eq!(page.next_before_sequence, None);

    let range = ok(store.session_event_range(&SessionEventRangeQuery {
        session_id: session_a.clone(),
        after_sequence: Some(1),
        up_to_sequence: Some(4),
        kinds: vec![ta_protocol::wire::DaemonEventKind::Approval],
    }));
    assert_eq!(
        range
            .records
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![3]
    );
    assert_eq!(range.latest_sequence, Some(3));
}

#[test]
fn read_run_events_filters_after_sequence_and_limits_results() {
    let session_id = SessionId::new("session-a").expect("session id");
    let run_id = RunId::new("run-a").expect("run id");
    let other_run_id = RunId::new("run-b").expect("run id");
    let mut store = InMemoryStore::current();

    for (sequence, event_run_id) in [
        (1, run_id.clone()),
        (2, other_run_id),
        (3, run_id.clone()),
        (4, run_id.clone()),
    ] {
        store
            .append_event(EventRecord {
                sequence,
                session_id: session_id.clone(),
                occurred_at_ms: sequence * 10,
                payload: DaemonEvent::Run(RunEvent {
                    run_id: event_run_id,
                    status: RunStatus::Running,
                    detail: format!("event {sequence}"),
                    output_contract: None,
                    recipe_id: None,
                    result: None,
                }),
            })
            .expect("run event");
    }

    let range = ok(store.read_run_events(&RunEventRangeQuery {
        session_id,
        run_id,
        after_sequence: Some(1),
        limit: 1,
    }));

    assert_eq!(
        range
            .records
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![3]
    );
    assert_eq!(range.latest_sequence, Some(4));
}

#[test]
fn approval_lookup_distinguishes_pending_resolved_and_not_found() {
    let session_id = SessionId::new("session-a").expect("session id");
    let run_id = RunId::new("run-a").expect("run id");
    let pending_id = ApprovalId::new("approval-pending").expect("approval id");
    let resolved_id = ApprovalId::new("approval-resolved").expect("approval id");
    let mut store = InMemoryStore::current();

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
                    ta_protocol::wire::ApprovalDecision::Approved,
                    ta_protocol::wire::ApprovalResolutionReason::User,
                    ta_protocol::wire::ApprovalActor::new("principal-memory-tests")
                        .expect("approval actor"),
                    None,
                ),
            }),
        })
        .expect("resolved event");

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
}
