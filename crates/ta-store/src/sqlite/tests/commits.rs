use super::*;
use crate::{
    AuthProfileRepository, ClaimScheduledWorkOccurrence, CommitRepository,
    CommitSessionOpenWithNavigation, NavigationConversationMetadata, NavigationRepository,
    NavigationState, PrincipalRepository, ReserveScheduledWorkOccurrence, ScheduledWorkRepository,
};
use ta_protocol::wire::ConversationPlacement;

fn auth_profile_with_preferences(
    id: &str,
    auth_method_id: &str,
    provider_id: &str,
    label: &str,
    order: u32,
    is_default: bool,
) -> crate::AuthProfileProjection {
    let mut profile = crate::connected_test_auth_profile(id, auth_method_id, provider_id);
    profile.profile.preferences = ta_protocol::wire::AuthProfilePreferences {
        label: label.to_string(),
        order,
        is_default,
    };
    profile
}

#[test]
fn commit_session_open_with_navigation_persists_session_and_navigation_together() {
    let path = test_db_path("commit-session-open-with-navigation");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-open-with-navigation").expect("session id");
    let owner_principal_id = "principal-test-owner".to_string();
    PrincipalRepository::save_principal(
        &mut store,
        PrincipalProjection {
            id: owner_principal_id.clone(),
            client_name: "sqlite-tests".to_string(),
            credential_hash: "credential-hash".to_string(),
        },
    )
    .expect("seed principal");
    crate::WorkspaceRepository::upsert_workspace(&mut store, crate::default_test_workspace())
        .expect("seed workspace");
    let navigation = NavigationState {
        conversations: vec![NavigationConversationMetadata {
            session_id: session_id.clone(),
            placement: ConversationPlacement::Temporary,
            archived: false,
            pinned: false,
        }],
        ..NavigationState::default()
    };

    store
        .commit_session_open_with_navigation(CommitSessionOpenWithNavigation {
            session: SessionProjection {
                id: session_id.clone(),
                owner_client_name: "sqlite-tests".to_string(),
                owner_principal_id: owner_principal_id.clone(),
                current_session_authority_hash: "session-authority-hash".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Temporary".to_string(),
                status: SessionStatus::Idle,
                workspace_id: crate::default_test_workspace_id(),
                next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
            },
            owner_principal_id: owner_principal_id.clone(),
            navigation: navigation.clone(),
            occurred_at_ms: 20,
        })
        .expect("session and navigation should commit");

    assert_eq!(some(store.session(&session_id)).title, "Temporary");
    assert_eq!(
        store
            .navigation_state(&owner_principal_id)
            .expect("navigation should persist"),
        navigation
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_session_open_with_navigation_rejects_unknown_workspace_without_navigation_mutation() {
    let path = test_db_path("commit-session-open-rejected");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-open-rejected").expect("session id");
    let owner_principal_id = "principal-test-owner".to_string();
    PrincipalRepository::save_principal(
        &mut store,
        PrincipalProjection {
            id: owner_principal_id.clone(),
            client_name: "sqlite-tests".to_string(),
            credential_hash: "credential-hash-rejected".to_string(),
        },
    )
    .expect("seed principal");
    let navigation = NavigationState {
        conversations: vec![NavigationConversationMetadata {
            session_id: session_id.clone(),
            placement: ConversationPlacement::Temporary,
            archived: false,
            pinned: false,
        }],
        ..NavigationState::default()
    };

    let error = store
        .commit_session_open_with_navigation(CommitSessionOpenWithNavigation {
            session: SessionProjection {
                id: session_id.clone(),
                owner_client_name: "sqlite-tests".to_string(),
                owner_principal_id: owner_principal_id.clone(),
                current_session_authority_hash: "session-authority-hash".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Rejected".to_string(),
                status: SessionStatus::Idle,
                workspace_id: ta_protocol::wire::WorkspaceId::new("workspace-missing")
                    .expect("workspace id"),
                next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
            },
            owner_principal_id: owner_principal_id.clone(),
            navigation,
            occurred_at_ms: 20,
        })
        .expect_err("unknown workspace should reject atomically");

    assert!(matches!(error, StoreError::SessionWorkspaceMissing { .. }));
    assert!(store.session(&session_id).expect("session read").is_none());
    assert_eq!(
        store
            .navigation_state(&owner_principal_id)
            .expect("navigation read"),
        NavigationState::default()
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn replace_auth_profile_preferences_reorders_its_group_atomically_in_sqlite() {
    let path = test_db_path("auth-profile-preferences");
    let mut store = SqliteStore::open(&path).expect("store should open");
    for profile in [
        auth_profile_with_preferences("profile-a", "method", "provider", "A", 0, true),
        auth_profile_with_preferences("profile-b", "method", "provider", "B", 1, false),
        auth_profile_with_preferences("profile-c", "method", "provider", "C", 2, false),
        auth_profile_with_preferences(
            "profile-other",
            "other-method",
            "provider",
            "Other",
            0,
            true,
        ),
    ] {
        store
            .save_auth_profile(profile)
            .expect("profile should persist");
    }

    store
        .replace_auth_profile_preferences(
            &ta_protocol::wire::AuthProfileId::new("profile-b").expect("profile id"),
            ta_protocol::wire::AuthProfilePreferences {
                label: "Renamed B".to_string(),
                order: 0,
                is_default: true,
            },
        )
        .expect("group preference replacement should persist");
    let group = store
        .auth_profiles()
        .expect("profiles")
        .into_iter()
        .filter(|profile| profile.auth_method_id().as_str() == "method")
        .map(|profile| {
            (
                profile.id().as_str().to_string(),
                profile.profile.preferences.label,
                profile.profile.preferences.order,
                profile.profile.preferences.is_default,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        group,
        vec![
            ("profile-b".to_string(), "Renamed B".to_string(), 0, true),
            ("profile-a".to_string(), "A".to_string(), 1, false),
            ("profile-c".to_string(), "C".to_string(), 2, false),
        ]
    );

    store
        .replace_auth_profile_preferences(
            &ta_protocol::wire::AuthProfileId::new("profile-b").expect("profile id"),
            ta_protocol::wire::AuthProfilePreferences {
                label: "Renamed B".to_string(),
                order: 2,
                is_default: false,
            },
        )
        .expect("target default can be explicitly cleared");
    assert!(
        store
            .auth_profiles()
            .expect("profiles")
            .into_iter()
            .filter(|profile| profile.auth_method_id().as_str() == "method")
            .all(|profile| !profile.profile.preferences.is_default)
    );

    let before_invalid_order = store.auth_profiles().expect("profiles");
    let error = store
        .replace_auth_profile_preferences(
            &ta_protocol::wire::AuthProfileId::new("profile-b").expect("profile id"),
            ta_protocol::wire::AuthProfilePreferences {
                label: "Invalid".to_string(),
                order: 3,
                is_default: false,
            },
        )
        .expect_err("out-of-range order must reject");
    assert!(matches!(
        error,
        StoreError::AuthProfilePreferenceOrderOutOfRange {
            order: 3,
            group_len: 3
        }
    ));
    assert_eq!(
        store.auth_profiles().expect("profiles"),
        before_invalid_order
    );

    drop(store);
    let store = SqliteStore::open(&path).expect("store should reopen");
    let restored_group = store
        .auth_profiles()
        .expect("profiles should survive restart")
        .into_iter()
        .filter(|profile| profile.auth_method_id().as_str() == "method")
        .map(|profile| {
            (
                profile.id().as_str().to_string(),
                profile.profile.preferences.label,
                profile.profile.preferences.order,
                profile.profile.preferences.is_default,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        restored_group,
        vec![
            ("profile-a".to_string(), "A".to_string(), 0, false),
            ("profile-c".to_string(), "C".to_string(), 1, false),
            ("profile-b".to_string(), "Renamed B".to_string(), 2, false),
        ]
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_persists_atomically() {
    let path = test_db_path("commit-run");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    store
        .save_session(SessionProjection {
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
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");

    let committed = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship store boundary".to_string(),
                status: RunStatus::Running,
                source: crate::default_test_run_source(),
                execution_context: crate::default_test_execution_context(),
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
            user_turn: crate::UserTurnCommit::NoUserTurn,
            events: vec![DaemonEvent::Run(
                ta_protocol::wire::RunEvent::active(
                    run_id.clone(),
                    RunStatus::Running,
                    None,
                    None,
                    None,
                )
                .expect("active status"),
            )],
            occurred_at_ms: 20,
            auth_profile_mutation: crate::AuthProfileCommitMutation::Unchanged,
        })
        .expect("run should commit");

    assert_eq!(committed.commit.id, 1);
    assert_eq!(committed.commit.first_sequence, 1);
    assert_eq!(committed.commit.last_sequence, 1);
    assert_eq!(committed.session.status, SessionStatus::Running);
    assert_eq!(some(store.run(&run_id)).status, RunStatus::Running);
    assert_eq!(
        some(store.session(&session_id)).status,
        SessionStatus::Running
    );
    assert_eq!(
        ok(store.session_event_page(&crate::SessionEventPageQuery {
            session_id: session_id.clone(),
            before_sequence: None,
            limit: 10,
            kinds: vec![DaemonEventKind::Run],
        }))
        .records
        .len(),
        1
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_persists_routed_profile_exhaustion_atomically() {
    let path = test_db_path("commit-exhaustion");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-exhausted").expect("session id");
    let run_id = RunId::new("run-exhausted").expect("run id");
    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Exhaustion".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session");
    store
        .save_auth_profile(crate::connected_test_auth_profile(
            "profile-test",
            "method-test",
            "provider-test",
        ))
        .expect("profile");

    store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-test")
                    .expect("runtime profile"),
                objective: "exhaustion".to_string(),
                status: RunStatus::Failed,
                source: crate::default_test_run_source(),
                execution_context: crate::default_test_execution_context(),
                harness: RunHarnessKind::Native,
                result: None,
                contract_violation: None,
                started_at_ms: None,
                ended_at_ms: None,
                last_event_seq: None,
                workspace_info: None,
                claimed_files: Vec::new(),
                conflict_summary: None,
            },
            user_turn: crate::UserTurnCommit::NoUserTurn,
            events: vec![DaemonEvent::Run(
                ta_protocol::wire::RunEvent::terminal_with_auth_profile_exhaustion(
                    run_id.clone(),
                    ta_protocol::wire::RunStatusReason::new(
                        "The selected account has exhausted its credits.",
                    )
                    .expect("reason"),
                    ta_protocol::wire::AuthProfileExhaustion::CreditsExhausted,
                )
                .expect("status"),
            )],
            occurred_at_ms: 20,
            auth_profile_mutation: crate::AuthProfileCommitMutation::SetExhausted {
                auth_profile_id: ta_protocol::wire::AuthProfileId::new("profile-test")
                    .expect("profile id"),
                exhaustion: ta_protocol::wire::AuthProfileExhaustion::CreditsExhausted,
            },
        })
        .expect("atomic exhaustion commit");

    assert_eq!(
        some(store.auth_profile(
            &ta_protocol::wire::AuthProfileId::new("profile-test").expect("profile id"),
        ))
        .profile
        .exhaustion,
        Some(ta_protocol::wire::AuthProfileExhaustion::CreditsExhausted)
    );
    assert_eq!(some(store.run(&run_id)).status, RunStatus::Failed);
}

#[test]
fn commit_run_transition_persists_only_durable_agent_stream_frames() {
    let path = test_db_path("commit-run-agent-stream-durable");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    store
        .save_session(SessionProjection {
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
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");

    let committed = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "stream".to_string(),
                status: RunStatus::Running,
                source: crate::default_test_run_source(),
                execution_context: crate::default_test_execution_context(),
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
            user_turn: crate::UserTurnCommit::NoUserTurn,
            events: vec![
                agent_stream_event(
                    &run_id,
                    AgentStreamFrame::ToolCallStarted {
                        tool_name: "shell".to_string(),
                        input: "{}".to_string(),
                    },
                ),
                agent_stream_event(
                    &run_id,
                    AgentStreamFrame::ToolCallProgressed {
                        delta: "stdout".to_string(),
                    },
                ),
                agent_stream_event(
                    &run_id,
                    AgentStreamFrame::ToolCallCompleted {
                        outcome: AgentToolCallOutcome::Completed,
                    },
                ),
            ],
            occurred_at_ms: 20,
            auth_profile_mutation: crate::AuthProfileCommitMutation::Unchanged,
        })
        .expect("run should commit");

    assert_eq!(
        committed
            .events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        committed
            .persisted_events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(committed.commit.first_sequence, 1);
    assert_eq!(committed.commit.last_sequence, 3);
    assert_eq!(
        ok(store.session_event_page(&crate::SessionEventPageQuery {
            session_id: session_id.clone(),
            before_sequence: None,
            limit: 10,
            kinds: vec![DaemonEventKind::AgentStream],
        }))
        .records
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>(),
        vec![3, 1]
    );
    assert_eq!(
        ok(store.session_event_range(&crate::SessionEventRangeQuery {
            session_id: session_id.clone(),
            after_sequence: None,
            up_to_sequence: None,
            kinds: vec![DaemonEventKind::AgentStream],
        }))
        .records
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>(),
        vec![1, 3]
    );

    let reopened = SqliteStore::open(&path).expect("store should reopen");
    assert_eq!(
        ok(reopened.session_event_page(&crate::SessionEventPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
            kinds: vec![DaemonEventKind::AgentStream],
        }))
        .records
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>(),
        vec![3, 1]
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_rolls_back_when_existing_run_projection_is_corrupt() {
    let path = test_db_path("commit-run-existing-corrupt");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    store
        .save_session(SessionProjection {
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
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");
    store
        .save_run(RunProjection {
            id: RunId::new("run-corrupt").expect("run id"),
            session_id: session_id.clone(),
            runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                .expect("runtime profile id"),
            objective: "Corrupt me".to_string(),
            status: RunStatus::Running,
            source: crate::default_test_run_source(),
            execution_context: crate::default_test_execution_context(),
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
        .expect("run should persist");
    store
        .conn
        .execute(
            "UPDATE runs SET data_json = ?1 WHERE id = ?2",
            params!["{", "run-corrupt"],
        )
        .expect("corrupt existing run json");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-new").expect("run id"),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Should roll back".to_string(),
                status: RunStatus::Running,
                source: crate::default_test_run_source(),
                execution_context: crate::default_test_execution_context(),
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
            user_turn: crate::UserTurnCommit::NoUserTurn,
            events: vec![DaemonEvent::Run(
                ta_protocol::wire::RunEvent::active(
                    RunId::new("run-new").expect("run id"),
                    RunStatus::Running,
                    None,
                    None,
                    None,
                )
                .expect("active status"),
            )],
            occurred_at_ms: 20,
            auth_profile_mutation: crate::AuthProfileCommitMutation::Unchanged,
        })
        .expect_err("commit should fail on corrupt existing run projection");

    assert!(matches!(
        error,
        StoreError::DecodeRecord {
            entity: "run_projection",
            ..
        }
    ));
    assert!(ok(store.run(&RunId::new("run-new").expect("run id"))).is_none());
    assert_eq!(some(store.session(&session_id)).status, SessionStatus::Idle);
    let event_count: i64 = store
        .conn
        .query_row("SELECT COUNT(1) FROM events", [], |row| row.get(0))
        .expect("event count");
    let commit_count: i64 = store
        .conn
        .query_row("SELECT COUNT(1) FROM commits", [], |row| row.get(0))
        .expect("commit count");
    assert_eq!(event_count, 0);
    assert_eq!(commit_count, 0);

    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_rejects_cross_session_run_projection() {
    let path = test_db_path("commit-run-cross-session");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    store
        .save_session(SessionProjection {
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
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-1").expect("run id"),
                session_id: SessionId::new("session-2").expect("session id"),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship store boundary".to_string(),
                status: RunStatus::Running,
                source: crate::default_test_run_source(),
                execution_context: crate::default_test_execution_context(),
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
            user_turn: crate::UserTurnCommit::NoUserTurn,
            events: vec![DaemonEvent::Run(
                ta_protocol::wire::RunEvent::active(
                    RunId::new("run-1").expect("run id"),
                    RunStatus::Running,
                    None,
                    None,
                    None,
                )
                .expect("active status"),
            )],
            occurred_at_ms: 20,
            auth_profile_mutation: crate::AuthProfileCommitMutation::Unchanged,
        })
        .expect_err("cross-session run commit must fail");

    assert_eq!(
        error,
        StoreError::CommitSessionMismatch {
            entity: "run",
            expected: "session-1".to_string(),
            actual: "session-2".to_string(),
        }
    );
    assert!(ok(store.runs()).is_empty());
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_rejects_orphan_approval_resolution() {
    let path = test_db_path("commit-run-orphan-approval-resolution");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    store
        .save_session(SessionProjection {
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
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");

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
                source: crate::default_test_run_source(),
                execution_context: crate::default_test_execution_context(),
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
            user_turn: crate::UserTurnCommit::NoUserTurn,
            events: vec![DaemonEvent::Approval(ApprovalEvent::Resolved {
                resolution: ta_protocol::wire::ApprovalResolution::new(
                    ApprovalId::new("approval-1").expect("approval id"),
                    run_id,
                    ApprovalDecision::Approved,
                    ta_protocol::wire::ApprovalResolutionReason::User,
                    ta_protocol::wire::ApprovalActor::new("principal-sqlite-tests")
                        .expect("approval actor"),
                    None,
                ),
            })],
            occurred_at_ms: 20,
            auth_profile_mutation: crate::AuthProfileCommitMutation::Unchanged,
        })
        .expect_err("orphan approval resolution must fail");

    assert_eq!(
        error,
        StoreError::ApprovalLifecycleViolation {
            approval_id: "approval-1".to_string(),
            detail: "approval resolution does not match a pending request".to_string(),
        }
    );
    assert!(ok(store.run(&RunId::new("run-1").expect("run id"))).is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_rejects_mismatched_run_event_run_id() {
    let path = test_db_path("commit-run-mismatched-event-run-id");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");
    store
        .save_session(SessionProjection {
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
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-1").expect("run id"),
                session_id,
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship store boundary".to_string(),
                status: RunStatus::Running,
                source: crate::default_test_run_source(),
                execution_context: crate::default_test_execution_context(),
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
            user_turn: crate::UserTurnCommit::NoUserTurn,
            events: vec![DaemonEvent::Run(
                ta_protocol::wire::RunEvent::active(
                    RunId::new("run-2").expect("run id"),
                    RunStatus::Running,
                    None,
                    None,
                    None,
                )
                .expect("active status"),
            )],
            occurred_at_ms: 20,
            auth_profile_mutation: crate::AuthProfileCommitMutation::Unchanged,
        })
        .expect_err("mismatched run event run id must fail");

    assert_eq!(
        error,
        StoreError::CommitRunEventMismatch {
            expected: "run-1".to_string(),
            actual: "run-2".to_string(),
        }
    );
    assert!(ok(store.run(&RunId::new("run-1").expect("run id"))).is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_run_transition_rejects_mismatched_agent_stream_run_id() {
    let path = test_db_path("commit-run-transition-agent-stream-mismatch");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");

    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Build daemon app server".to_string(),
            status: SessionStatus::Idle,
            workspace_id: crate::default_test_workspace_id(),
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session");

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-1").expect("run id"),
                session_id,
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Ship store boundary".to_string(),
                status: RunStatus::Running,
                source: crate::default_test_run_source(),
                execution_context: crate::default_test_execution_context(),
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
            user_turn: crate::UserTurnCommit::NoUserTurn,
            events: vec![DaemonEvent::AgentStream(AgentStreamEvent {
                run_id: RunId::new("run-2").expect("run id"),
                emission: ta_protocol::wire::StreamEmission {
                    turn_id: None,
                    item_id: None,
                    fragment_sequence: None,
                    frame: AgentStreamFrame::AssistantTurnStarted,
                },
            })],
            occurred_at_ms: 20,
            auth_profile_mutation: crate::AuthProfileCommitMutation::Unchanged,
        })
        .expect_err("mismatched agent stream run id must fail");

    assert_eq!(
        error,
        StoreError::CommitRunEventMismatch {
            expected: "run-1".to_string(),
            actual: "run-2".to_string(),
        }
    );
    assert!(ok(store.run(&RunId::new("run-1").expect("run id"))).is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn commit_session_open_persists_session_and_allocates_event_sequence() {
    let path = test_db_path("commit-session-open");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let session_id = SessionId::new("session-1").expect("session id");

    crate::WorkspaceRepository::upsert_workspace(&mut store, crate::default_test_workspace())
        .expect("seed workspace");
    let committed = store
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
                next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
            },
            occurred_at_ms: 20,
        })
        .expect("session should commit");

    assert_eq!(committed.commit.id, 1);
    assert_eq!(committed.event.sequence, 1);
    assert_eq!(some(store.session(&session_id)).title, "Persisted");
    assert_eq!(
        ok(store.session_event_page(&crate::SessionEventPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
            kinds: vec![DaemonEventKind::Session],
        }))
        .records
        .len(),
        1
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn reopen_preserves_committed_run_transition_and_continues_event_sequence() {
    let path = test_db_path("reopen-committed-run");
    {
        let mut store = SqliteStore::open(&path).expect("store should open");
        let session_id = SessionId::new("session-1").expect("session id");
        store
            .save_session(SessionProjection {
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
                next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
            })
            .expect("session should persist");
        store
            .commit_run_transition(CommitRunTransition {
                session_id: session_id.clone(),
                run: RunProjection {
                    id: RunId::new("run-1").expect("run id"),
                    session_id,
                    runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new(
                        "runtime-codex-safe",
                    )
                    .expect("runtime profile id"),
                    objective: "Ship store boundary".to_string(),
                    status: RunStatus::Running,
                    source: crate::default_test_run_source(),
                    execution_context: crate::default_test_execution_context(),
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
                user_turn: crate::UserTurnCommit::NoUserTurn,
                events: vec![DaemonEvent::Run(
                    ta_protocol::wire::RunEvent::active(
                        RunId::new("run-1").expect("run id"),
                        RunStatus::Running,
                        None,
                        None,
                        None,
                    )
                    .expect("active status"),
                )],
                occurred_at_ms: 20,
                auth_profile_mutation: crate::AuthProfileCommitMutation::Unchanged,
            })
            .expect("run should commit");
    }

    let mut reopened = SqliteStore::open(&path).expect("store should reopen");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    let second = reopened
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: ArtifactId::new("artifact-1").expect("artifact id"),
                session_id: session_id.clone(),
                run_id,
                kind: ArtifactKind::Patch,
                metadata: ta_protocol::wire::ArtifactMetadata::Standard,
                storage_path: "artifacts/run-1/patch.diff".to_string(),
            },
            occurred_at_ms: 30,
        })
        .expect("artifact should commit");

    assert_eq!(
        some(reopened.session(&session_id)).status,
        SessionStatus::Running
    );
    assert_eq!(second.event.sequence, 2);

    let _ = std::fs::remove_file(path);
}

#[test]
fn reopen_preserves_committed_checkpoint_and_next_event_sequence() {
    let path = test_db_path("reopen-committed-checkpoint");
    {
        let mut store = SqliteStore::open(&path).expect("store should open");
        let session_id = SessionId::new("session-1").expect("session id");
        let run_id = RunId::new("run-1").expect("run id");
        store
            .save_session(SessionProjection {
                id: session_id.clone(),
                owner_client_name: "sqlite-tests".to_string(),
                owner_principal_id: "principal-test-owner".to_string(),
                current_session_authority_hash: "session-authority-hash".to_string(),
                current_session_authority_generation: 0,
                recovery_session_authority_hash: None,
                recovery_session_authority_generation: None,
                title: "Persisted".to_string(),
                status: SessionStatus::Running,
                workspace_id: crate::default_test_workspace_id(),
                next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
            })
            .expect("session should persist");
        store
            .save_run(RunProjection {
                id: run_id.clone(),
                session_id,
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-codex-safe")
                    .expect("runtime profile id"),
                objective: "Persist checkpoint".to_string(),
                status: RunStatus::Running,
                source: crate::default_test_run_source(),
                execution_context: crate::default_test_execution_context(),
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
            .expect("run should persist");
        store
            .commit_checkpoint_persist(CommitCheckpointPersist {
                checkpoint: crate::test_checkpoint_record(run_id, 1),
                occurred_at_ms: 20,
            })
            .expect("checkpoint should commit");
    }

    let mut reopened = SqliteStore::open(&path).expect("store should reopen");
    let session_id = SessionId::new("session-1").expect("session id");
    let run_id = RunId::new("run-1").expect("run id");
    let artifact = reopened
        .commit_artifact_publish(CommitArtifactPublish {
            artifact: ArtifactRecord {
                id: ArtifactId::new("artifact-1").expect("artifact id"),
                session_id,
                run_id: run_id.clone(),
                kind: ArtifactKind::Patch,
                metadata: ta_protocol::wire::ArtifactMetadata::Standard,
                storage_path: "artifacts/run-1/patch.diff".to_string(),
            },
            occurred_at_ms: 30,
        })
        .expect("artifact should commit");

    assert_eq!(artifact.commit.id, 2);
    assert_eq!(artifact.event.sequence, 1);
    assert_eq!(
        ok(reopened.checkpoints_for_run(&run_id)),
        vec![crate::test_checkpoint_record(run_id, 1)]
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn startup_reconciliation_settles_scheduled_claimed_occurrence_atomically() {
    let path = test_db_path("startup-scheduled-terminal");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let context = crate::default_test_execution_context();
    let route = crate::default_test_run_source().route().clone();
    let scheduled_work_id =
        ta_protocol::wire::ScheduledWorkId::new("schedule-startup").expect("scheduled work id");
    let occurrence_id = ta_protocol::wire::ScheduledWorkOccurrenceId::new("occurrence-startup")
        .expect("occurrence id");
    let session_id = SessionId::new("session-startup-scheduled").expect("session id");
    let run_id = RunId::new("run-startup-scheduled").expect("run id");
    let definition = ta_protocol::wire::ScheduledWorkDefinition {
        id: scheduled_work_id.clone(),
        session_id: session_id.clone(),
        objective: "Terminalize opaque scheduled work".to_string(),
        route: route.clone(),
        execution_request: ta_protocol::wire::ScheduledWorkExecutionRequest {
            workspace_id: context.workspace_id.clone(),
            workspace_root: context.workspace_root.clone(),
            repo_root: context.workspace_root.clone(),
            artifact_root: context.artifact_root.clone(),
            workspace_mode: ta_protocol::wire::WorkspaceMode::WorkspaceWrite,
            cleanup_policy: ta_protocol::wire::WorktreeCleanupPolicy::DeleteOnSuccess,
            planned_write_files: Vec::new(),
            workspace_scope: context.workspace_scope.clone(),
            sandbox_profile: context.sandbox_profile.clone(),
            permission_policy: context.permission_policy,
            network_policy: context.network_policy.clone(),
            env_policy: context.env_policy.clone(),
        },
        due_at_ms: 10,
        attention_policy: ta_protocol::wire::ScheduledWorkAttentionPolicy::AttentionOnly,
    };
    let occurrence = ta_protocol::wire::ScheduledWorkOccurrence {
        id: occurrence_id.clone(),
        scheduled_work_id: scheduled_work_id.clone(),
        due_at_ms: 10,
        state: ta_protocol::wire::ScheduledWorkOccurrenceState::Pending,
    };
    let queued = RunProjection {
        id: run_id.clone(),
        session_id: session_id.clone(),
        runtime_profile_id: route.runtime_profile_id.clone(),
        objective: definition.objective.clone(),
        status: RunStatus::Queued,
        harness: route.harness,
        source: ta_protocol::wire::RunSource::ScheduledWork {
            route,
            scheduled_work_id: scheduled_work_id.clone(),
            occurrence_id: occurrence_id.clone(),
        },
        execution_context: context,
        result: None,
        contract_violation: None,
        started_at_ms: None,
        ended_at_ms: None,
        last_event_seq: None,
        workspace_info: None,
        claimed_files: Vec::new(),
        conflict_summary: None,
    };
    store
        .save_session(SessionProjection {
            id: session_id.clone(),
            owner_client_name: "sqlite-tests".to_string(),
            owner_principal_id: "principal-test-owner".to_string(),
            current_session_authority_hash: "session-authority-hash".to_string(),
            current_session_authority_generation: 0,
            recovery_session_authority_hash: None,
            recovery_session_authority_generation: None,
            title: "Scheduled startup".to_string(),
            status: SessionStatus::Running,
            workspace_id: crate::default_test_workspace_id(),
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session should persist");
    store
        .create_scheduled_work(definition, occurrence)
        .expect("scheduled work should persist");
    store
        .reserve_scheduled_work_occurrence(ReserveScheduledWorkOccurrence {
            scheduled_work_id,
            occurrence_id: occurrence_id.clone(),
            run_id: run_id.clone(),
        })
        .expect("occurrence should reserve");
    store
        .publish_prepared_scheduled_work_occurrence(ClaimScheduledWorkOccurrence {
            scheduled_work_id: ta_protocol::wire::ScheduledWorkId::new("schedule-startup")
                .expect("scheduled work id"),
            occurrence_id: occurrence_id.clone(),
            run: queued.clone(),
        })
        .expect("scheduled run should publish");
    let mut opaque_running = queued.clone();
    opaque_running.status = RunStatus::Running;
    store
        .save_run(opaque_running.clone())
        .expect("opaque run should persist");
    let mut failed = opaque_running;
    failed.status = RunStatus::Failed;

    store
        .commit_startup_reconciliation(crate::CommitStartupReconciliation {
            transitions: vec![crate::CommitRunTransition {
                session_id,
                run: failed.clone(),
                user_turn: crate::UserTurnCommit::NoUserTurn,
                events: vec![DaemonEvent::Run(
                    ta_protocol::wire::RunEvent::terminal(
                        run_id.clone(),
                        RunStatus::Failed,
                        ta_protocol::wire::RunStatusReason::new("daemon restarted")
                            .expect("reason"),
                        None,
                        None,
                        None,
                    )
                    .expect("terminal event"),
                )],
                occurred_at_ms: 20,
                auth_profile_mutation: crate::AuthProfileCommitMutation::Unchanged,
            }],
        })
        .expect("startup reconciliation should settle both records");

    assert_eq!(some(store.run(&run_id)).status, RunStatus::Failed);
    assert!(matches!(
        some(store.scheduled_work_occurrence(&occurrence_id)).state,
        ta_protocol::wire::ScheduledWorkOccurrenceState::Failed { run_id: settled }
            if settled == run_id
    ));
    let _ = std::fs::remove_file(path);
}
