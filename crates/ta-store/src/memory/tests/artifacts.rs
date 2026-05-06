use super::*;

#[test]
fn durable_activity_reads_include_session_scoped_artifact_events() {
    let session_a = SessionId::new("session-a").expect("session id");
    let session_b = SessionId::new("session-b").expect("session id");
    let run_a = RunId::new("run-a").expect("run id");
    let run_b = RunId::new("run-b").expect("run id");
    let artifact_a = ArtifactId::new("artifact-a").expect("artifact id");
    let artifact_b = ArtifactId::new("artifact-b").expect("artifact id");
    let mut store = InMemoryStore::current();

    store
        .append_event(EventRecord {
            sequence: 1,
            session_id: session_a.clone(),
            occurred_at_ms: 10,
            payload: DaemonEvent::Artifact(ArtifactEvent {
                artifact: ArtifactSummary {
                    id: artifact_a.clone(),
                    run_id: run_a.clone(),
                    kind: ArtifactKind::Patch,
                    storage_path: "artifacts/run-a/patch.diff".to_string(),
                },
            }),
        })
        .expect("session a artifact event");
    store
        .append_event(EventRecord {
            sequence: 2,
            session_id: session_b,
            occurred_at_ms: 20,
            payload: DaemonEvent::Artifact(ArtifactEvent {
                artifact: ArtifactSummary {
                    id: artifact_b,
                    run_id: run_b,
                    kind: ArtifactKind::Transcript,
                    storage_path: "artifacts/run-b/transcript.md".to_string(),
                },
            }),
        })
        .expect("session b artifact event");
    store
        .append_event(EventRecord {
            sequence: 3,
            session_id: session_a.clone(),
            occurred_at_ms: 30,
            payload: DaemonEvent::Artifact(ArtifactEvent {
                artifact: ArtifactSummary {
                    id: ArtifactId::new("artifact-c").expect("artifact id"),
                    run_id: run_a,
                    kind: ArtifactKind::Patch,
                    storage_path: "artifacts/run-a/patch-2.diff".to_string(),
                },
            }),
        })
        .expect("session a later artifact event");

    let page = ok(store.session_event_page(&SessionEventPageQuery {
        session_id: session_a,
        before_sequence: None,
        limit: 10,
        kinds: vec![ta_protocol::wire::DaemonEventKind::Artifact],
    }));

    assert_eq!(
        page.records
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![3, 1]
    );
    assert_eq!(page.latest_sequence, Some(3));
    assert_eq!(page.next_before_sequence, None);
    assert!(matches!(
        &page.records[0].payload,
        DaemonEvent::Artifact(ArtifactEvent { artifact })
            if artifact.id.as_str() == "artifact-c"
    ));
}

#[test]
fn artifact_reads_are_session_scoped_and_ordered() {
    let session_a = SessionId::new("session-a").expect("session id");
    let session_b = SessionId::new("session-b").expect("session id");
    let run_a = RunId::new("run-a").expect("run id");
    let run_b = RunId::new("run-b").expect("run id");
    let run_c = RunId::new("run-c").expect("run id");
    let artifact_a = ArtifactId::new("artifact-a").expect("artifact id");
    let artifact_b = ArtifactId::new("artifact-b").expect("artifact id");
    let artifact_c = ArtifactId::new("artifact-c").expect("artifact id");
    let mut store = InMemoryStore::current();

    store
        .save_run(RunProjection {
            id: run_a.clone(),
            session_id: session_a.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "a".to_string(),
            status: RunStatus::Running,
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
        .expect("run a projection");
    store
        .save_run(RunProjection {
            id: run_b.clone(),
            session_id: session_b.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "b".to_string(),
            status: RunStatus::Running,
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
        .expect("run b projection");
    store
        .save_run(RunProjection {
            id: run_c.clone(),
            session_id: session_a.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "c".to_string(),
            status: RunStatus::Running,
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
        .expect("run c projection");
    store
        .save_artifact(ArtifactRecord {
            id: artifact_c.clone(),
            session_id: session_a.clone(),
            run_id: run_c.clone(),
            kind: ArtifactKind::Transcript,
            storage_path: "artifacts/run-c/transcript.md".to_string(),
        })
        .expect("artifact c");
    store
        .save_artifact(ArtifactRecord {
            id: artifact_b.clone(),
            session_id: session_b.clone(),
            run_id: run_b.clone(),
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/run-b/patch.diff".to_string(),
        })
        .expect("artifact b");
    store
        .save_artifact(ArtifactRecord {
            id: artifact_a.clone(),
            session_id: session_a.clone(),
            run_id: run_a.clone(),
            kind: ArtifactKind::Patch,
            storage_path: "artifacts/run-a/patch.diff".to_string(),
        })
        .expect("artifact a");

    let persistence: &dyn PersistenceStore = &store;

    let artifacts = ok(persistence.artifacts_for_session(&SessionArtifactQuery {
        session_id: session_a.clone(),
        run_id: None,
        artifact_id: None,
    }));
    assert_eq!(
        artifacts
            .iter()
            .map(|artifact| artifact.id.clone())
            .collect::<Vec<_>>(),
        vec![artifact_a.clone(), artifact_c.clone()]
    );
    assert_eq!(artifacts[1].storage_path, "artifacts/run-c/transcript.md");
    assert_eq!(
        some(persistence.artifact_for_session(&SessionArtifactQuery {
            session_id: session_a.clone(),
            run_id: Some(run_a.clone()),
            artifact_id: Some(artifact_a.clone()),
        }))
        .storage_path,
        "artifacts/run-a/patch.diff"
    );
    assert_eq!(
        ok(persistence.artifact_for_session(&SessionArtifactQuery {
            session_id: session_b,
            run_id: None,
            artifact_id: Some(artifact_a),
        })),
        None
    );
    assert_eq!(
        ok(persistence.artifacts_for_session(&SessionArtifactQuery {
            session_id: session_a,
            run_id: Some(run_c),
            artifact_id: None,
        }))
        .len(),
        1
    );
    assert_eq!(ok(persistence.artifacts_for_run(&run_b)).len(), 1);
}

#[test]
fn commit_artifact_publish_persists_artifact_and_allocates_monotonic_event_sequence() {
    let session_id = SessionId::new("session-a").expect("session id");
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
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Running,
        })
        .expect("session");
    store
        .save_run(RunProjection {
            id: RunId::new("run-a").expect("run id"),
            session_id: session_id.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Ship patch".to_string(),
            status: RunStatus::Running,
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
        .save_run(RunProjection {
            id: RunId::new("run-b").expect("run id"),
            session_id: session_id.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Ship transcript".to_string(),
            status: RunStatus::Running,
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
            sequence: 4,
            session_id: session_id.clone(),
            occurred_at_ms: 10,
            payload: DaemonEvent::Session(SessionEvent {
                session_id: session_id.clone(),
                status: SessionStatus::Running,
            }),
        })
        .expect("seed event");

    let first = store
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: ArtifactId::new("artifact-a").expect("artifact id"),
                session_id: session_id.clone(),
                run_id: RunId::new("run-a").expect("run id"),
                kind: ArtifactKind::Patch,
                storage_path: "artifacts/run-a/patch.diff".to_string(),
            },
            occurred_at_ms: 20,
        })
        .expect("first artifact event");
    let second = store
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: ArtifactId::new("artifact-b").expect("artifact id"),
                session_id: session_id.clone(),
                run_id: RunId::new("run-b").expect("run id"),
                kind: ArtifactKind::Transcript,
                storage_path: "artifacts/run-b/transcript.md".to_string(),
            },
            occurred_at_ms: 30,
        })
        .expect("second artifact event");

    assert_eq!(first.event.sequence, 5);
    assert_eq!(second.event.sequence, 6);
    assert!(matches!(
        &second.event.payload,
        DaemonEvent::Artifact(ArtifactEvent { artifact })
            if artifact.id.as_str() == "artifact-b"
    ));
    assert_eq!(
        some(store.artifact_for_session(&SessionArtifactQuery {
            session_id: session_id.clone(),
            run_id: None,
            artifact_id: Some(ArtifactId::new("artifact-b").expect("artifact id")),
        }))
        .storage_path,
        "artifacts/run-b/transcript.md"
    );
    assert_eq!(
        ok(store.session_event_page(&SessionEventPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
            kinds: vec![ta_protocol::wire::DaemonEventKind::Artifact],
        }))
        .records
        .iter()
        .map(|record| record.sequence)
        .collect::<Vec<_>>(),
        vec![6, 5]
    );
}

#[test]
fn commit_run_transition_rejects_cross_session_run_projection() {
    let session_id = SessionId::new("session-a").expect("session id");
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
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Idle,
        })
        .expect("session");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-a").expect("run id"),
                session_id: SessionId::new("session-b").expect("session id"),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship app server hard cut".to_string(),
                status: RunStatus::Running,
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
            },
            events: vec![DaemonEvent::Run(RunEvent {
                run_id: RunId::new("run-a").expect("run id"),
                status: RunStatus::Running,
                detail: "Execution started".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            })],
            occurred_at_ms: 20,
        })
        .expect_err("cross-session run commit must fail");

    assert_eq!(
        error,
        StoreError::CommitSessionMismatch {
            entity: "run",
            expected: "session-a".to_string(),
            actual: "session-b".to_string(),
        }
    );
    assert!(ok(store.runs()).is_empty());
}

#[test]
fn commit_run_transition_rejects_orphan_approval_resolution() {
    let session_id = SessionId::new("session-a").expect("session id");
    let run_id = RunId::new("run-a").expect("run id");
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
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Idle,
        })
        .expect("session");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship app server hard cut".to_string(),
                status: RunStatus::Running,
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
            },
            events: vec![DaemonEvent::Approval(ApprovalEvent::Resolved {
                resolution: ta_protocol::wire::ApprovalResolution::new(
                    ApprovalId::new("approval-a").expect("approval id"),
                    run_id,
                    ta_protocol::wire::ApprovalDecision::Approved,
                    ta_protocol::wire::ApprovalResolutionReason::User,
                    ta_protocol::wire::ApprovalActor::new("principal-memory-tests")
                        .expect("approval actor"),
                    None,
                ),
            })],
            occurred_at_ms: 20,
        })
        .expect_err("orphan approval resolution must fail");

    assert_eq!(
        error,
        StoreError::ApprovalLifecycleViolation {
            approval_id: "approval-a".to_string(),
            detail: "approval resolution does not match a pending request".to_string(),
        }
    );
    assert!(ok(store.runs()).is_empty());
}

#[test]
fn commit_run_transition_rejects_mismatched_run_event_run_id() {
    let session_id = SessionId::new("session-a").expect("session id");
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
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Idle,
        })
        .expect("session");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-a").expect("run id"),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship app server hard cut".to_string(),
                status: RunStatus::Running,
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
            },
            events: vec![DaemonEvent::Run(RunEvent {
                run_id: RunId::new("run-b").expect("run id"),
                status: RunStatus::Running,
                detail: "Execution started".to_string(),
                output_contract: None,
                recipe_id: None,
                result: None,
            })],
            occurred_at_ms: 20,
        })
        .expect_err("mismatched run event run id must fail");

    assert_eq!(
        error,
        StoreError::CommitRunEventMismatch {
            expected: "run-a".to_string(),
            actual: "run-b".to_string(),
        }
    );
    assert!(ok(store.runs()).is_empty());
}

#[test]
fn commit_run_transition_rejects_mismatched_agent_stream_run_id() {
    let session_id = SessionId::new("session-a").expect("session id");
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
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Idle,
        })
        .expect("session");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-a").expect("run id"),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship app server hard cut".to_string(),
                status: RunStatus::Running,
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
            },
            events: vec![DaemonEvent::AgentStream(AgentStreamEvent {
                run_id: RunId::new("run-b").expect("run id"),
                emission: ta_protocol::wire::StreamEmission {
                    turn_id: None,
                    item_id: None,
                    fragment_sequence: None,
                    frame: AgentStreamFrame::AssistantTurnStarted,
                },
            })],
            occurred_at_ms: 20,
        })
        .expect_err("mismatched agent stream run id must fail");

    assert_eq!(
        error,
        StoreError::CommitRunEventMismatch {
            expected: "run-a".to_string(),
            actual: "run-b".to_string(),
        }
    );
    assert!(ok(store.runs()).is_empty());
}

#[test]
fn commit_artifact_publish_rejects_cross_session_run_projection() {
    let session_id = SessionId::new("session-a").expect("session id");
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
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Running,
        })
        .expect("session");
    store
        .save_session(SessionProjection {
            id: SessionId::new("session-b").expect("session id"),
            owner_client_name: "memory-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Other".to_string(),
            status: SessionStatus::Running,
        })
        .expect("session");
    store
        .save_run(RunProjection {
            id: RunId::new("run-a").expect("run id"),
            session_id: SessionId::new("session-b").expect("session id"),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Ship patch".to_string(),
            status: RunStatus::Running,
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

    let error = store
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: ArtifactId::new("artifact-a").expect("artifact id"),
                session_id: session_id.clone(),
                run_id: RunId::new("run-a").expect("run id"),
                kind: ArtifactKind::Patch,
                storage_path: "artifacts/run-a/patch.diff".to_string(),
            },
            occurred_at_ms: 20,
        })
        .expect_err("cross-session artifact commit must fail");

    assert_eq!(
        error,
        StoreError::CommitSessionMismatch {
            entity: "artifact",
            expected: "session-b".to_string(),
            actual: "session-a".to_string(),
        }
    );
    assert!(
        ok(store.artifacts_for_session(&SessionArtifactQuery {
            session_id,
            run_id: None,
            artifact_id: None,
        }))
        .is_empty()
    );
}

#[test]
fn commit_artifact_publish_rejects_non_running_run_projection() {
    let session_id = SessionId::new("session-a").expect("session id");
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
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Idle,
        })
        .expect("session");
    store
        .save_run(RunProjection {
            id: RunId::new("run-a").expect("run id"),
            session_id: session_id.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Ship patch".to_string(),
            status: RunStatus::Cancelled,
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

    let error = store
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: ArtifactId::new("artifact-a").expect("artifact id"),
                session_id: session_id.clone(),
                run_id: RunId::new("run-a").expect("run id"),
                kind: ArtifactKind::Patch,
                storage_path: "artifacts/run-a/patch.diff".to_string(),
            },
            occurred_at_ms: 20,
        })
        .expect_err("non-running artifact commit must fail");

    assert_eq!(
        error,
        StoreError::CommitRunStatusMismatch {
            entity: "artifact",
            expected: RunStatus::Running,
            actual: RunStatus::Cancelled,
        }
    );
    assert!(
        ok(store.artifacts_for_session(&SessionArtifactQuery {
            session_id,
            run_id: None,
            artifact_id: None,
        }))
        .is_empty()
    );
}
