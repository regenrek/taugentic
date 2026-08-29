use super::*;
use crate::{
    AuthProfileRepository, CommitRepository, CommitSessionOpenWithNavigation,
    NavigationConversationMetadata, NavigationRepository, NavigationState,
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
    let session_id = SessionId::new("session-open-with-navigation").expect("session id");
    let owner_principal_id = "principal-test-owner".to_string();
    let mut store = InMemoryStore::current();
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
                owner_client_name: "memory-tests".to_string(),
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
}

#[test]
fn commit_session_open_with_navigation_rejects_unknown_workspace_without_navigation_mutation() {
    let session_id = SessionId::new("session-open-rejected").expect("session id");
    let owner_principal_id = "principal-test-owner".to_string();
    let mut store = InMemoryStore::current();
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
                owner_client_name: "memory-tests".to_string(),
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
}

#[test]
fn replace_auth_profile_preferences_reorders_only_its_group_without_partial_mutation() {
    let mut store = InMemoryStore::current();
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
    assert_eq!(
        some(store.auth_profile(
            &ta_protocol::wire::AuthProfileId::new("profile-other").expect("profile id"),
        ))
        .profile
        .preferences,
        ta_protocol::wire::AuthProfilePreferences {
            label: "Other".to_string(),
            order: 0,
            is_default: true,
        }
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
}

#[test]
fn commit_run_transition_updates_session() {
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
            workspace_id: crate::default_test_workspace_id(),
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session");
    store
        .append_event(EventRecord {
            sequence: 4,
            session_id: session_id.clone(),
            occurred_at_ms: 10,
            payload: DaemonEvent::Session(SessionEvent {
                session_id: session_id.clone(),
                status: SessionStatus::Idle,
            }),
        })
        .expect("seed event");

    let committed = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: RunId::new("run-a").expect("run id"),
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
            events: vec![DaemonEvent::Run(
                RunEvent::active(
                    RunId::new("run-a").expect("run id"),
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
        .expect("run start event");
    let event = committed.events.first().expect("run start event");

    assert_eq!(event.sequence, 5);
    assert!(matches!(
        &event.payload,
        DaemonEvent::Run(RunEvent::Status(event))
            if event.run_id().as_str() == "run-a"
                && event.status() == RunStatus::Running
    ));
    assert_eq!(
        some(store.session(&session_id)).status,
        SessionStatus::Running
    );
    assert_eq!(
        some(store.run(&RunId::new("run-a").expect("run id"))).objective,
        "Ship app server hard cut"
    );
}

#[test]
fn commit_run_transition_marks_only_the_routed_profile_exhausted() {
    let session_id = SessionId::new("session-exhausted").expect("session id");
    let run_id = RunId::new("run-exhausted").expect("run id");
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
                RunEvent::terminal_with_auth_profile_exhaustion(
                    run_id.clone(),
                    ta_protocol::wire::RunStatusReason::new(
                        "The selected account is rate limited.",
                    )
                    .expect("reason"),
                    ta_protocol::wire::AuthProfileExhaustion::RateLimited,
                )
                .expect("status"),
            )],
            occurred_at_ms: 20,
            auth_profile_mutation: crate::AuthProfileCommitMutation::SetExhausted {
                auth_profile_id: ta_protocol::wire::AuthProfileId::new("profile-test")
                    .expect("profile id"),
                exhaustion: ta_protocol::wire::AuthProfileExhaustion::RateLimited,
            },
        })
        .expect("atomic exhaustion commit");

    assert_eq!(
        some(store.auth_profile(
            &ta_protocol::wire::AuthProfileId::new("profile-test").expect("profile id"),
        ))
        .profile
        .exhaustion,
        Some(ta_protocol::wire::AuthProfileExhaustion::RateLimited)
    );
    assert_eq!(some(store.run(&run_id)).status, RunStatus::Failed);
}

#[test]
fn commit_run_transition_rejects_mismatched_exhaustion_profile_without_mutation() {
    let session_id = SessionId::new("session-profile-mismatch").expect("session id");
    let run_id = RunId::new("run-profile-mismatch").expect("run id");
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
            title: "Mismatch".to_string(),
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

    let error = store
        .commit_run_transition(CommitRunTransition {
            session_id: session_id.clone(),
            run: RunProjection {
                id: run_id.clone(),
                session_id: session_id.clone(),
                runtime_profile_id: ta_protocol::wire::RuntimeProfileId::new("runtime-test")
                    .expect("runtime profile"),
                objective: "mismatch".to_string(),
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
                RunEvent::terminal_with_auth_profile_exhaustion(
                    run_id.clone(),
                    ta_protocol::wire::RunStatusReason::new(
                        "The selected account is rate limited.",
                    )
                    .expect("reason"),
                    ta_protocol::wire::AuthProfileExhaustion::RateLimited,
                )
                .expect("status"),
            )],
            occurred_at_ms: 20,
            auth_profile_mutation: crate::AuthProfileCommitMutation::SetExhausted {
                auth_profile_id: ta_protocol::wire::AuthProfileId::new("profile-other")
                    .expect("profile id"),
                exhaustion: ta_protocol::wire::AuthProfileExhaustion::RateLimited,
            },
        })
        .expect_err("route mismatch must reject");

    assert!(matches!(
        error,
        StoreError::AuthProfileMutationRouteMismatch { .. }
    ));
    assert!(store.run(&run_id).expect("run lookup").is_none());
    assert!(
        store
            .events_for_session(&session_id)
            .expect("events")
            .is_empty()
    );
    assert_eq!(
        some(store.auth_profile(
            &ta_protocol::wire::AuthProfileId::new("profile-test").expect("profile id"),
        ))
        .profile
        .exhaustion,
        None
    );
}

#[test]
fn commit_run_transition_persists_only_durable_agent_stream_frames() {
    let session_id = SessionId::new("session-lane").expect("session id");
    let run_id = RunId::new("run-lane").expect("run id");
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
            title: "Lane".to_string(),
            status: SessionStatus::Running,
            workspace_id: crate::default_test_workspace_id(),
            next_run_selection: ta_protocol::wire::SessionNextRunSelection::Unselected,
        })
        .expect("session");

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
        .expect("run transition should commit");

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
        ok(store.events_for_session(&session_id))
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(
        ok(store.session_event_range(&SessionEventRangeQuery {
            session_id: session_id.clone(),
            after_sequence: None,
            up_to_sequence: None,
            kinds: vec![ta_protocol::wire::DaemonEventKind::AgentStream],
        }))
        .records
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(
        ok(store.session_event_page(&SessionEventPageQuery {
            session_id,
            before_sequence: None,
            limit: 10,
            kinds: vec![ta_protocol::wire::DaemonEventKind::AgentStream],
        }))
        .records
        .iter()
        .map(|event| event.sequence)
        .collect::<Vec<_>>(),
        vec![3, 1]
    );
}
