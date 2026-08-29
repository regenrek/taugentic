use super::*;
use crate::{
    AuthProfileCommitMutation, ClaimScheduledWorkOccurrence, CommitRepository, CommitRunTransition,
    ReserveScheduledWorkOccurrence, ScheduledWorkRepository, StoreSeedRepository, UserTurnCommit,
};
use std::sync::atomic::{AtomicU64, Ordering};
use ta_protocol::wire::{
    DaemonEvent, RunEvent, RunHarnessKind, RunId, RunSource, RunStatus, RunStatusReason,
    RuntimeProfileId, ScheduledWorkAttentionPolicy, ScheduledWorkDefinition,
    ScheduledWorkExecutionRequest, ScheduledWorkId, ScheduledWorkOccurrence,
    ScheduledWorkOccurrenceId, ScheduledWorkOccurrenceState, SessionId, SessionNextRunSelection,
};

fn session(id: SessionId) -> SessionProjection {
    SessionProjection {
        id,
        owner_client_name: "scheduled-work-test".to_string(),
        owner_principal_id: "principal".to_string(),
        current_session_authority_hash: "authority".to_string(),
        current_session_authority_generation: 0,
        recovery_session_authority_hash: None,
        recovery_session_authority_generation: None,
        title: "Scheduled work".to_string(),
        status: SessionStatus::Idle,
        workspace_id: crate::default_test_workspace_id(),
        next_run_selection: SessionNextRunSelection::Unselected,
    }
}

fn fixture() -> (
    ScheduledWorkDefinition,
    ScheduledWorkOccurrence,
    RunProjection,
) {
    let context = crate::default_test_execution_context();
    let route = crate::default_test_run_source().route().clone();
    let definition = ScheduledWorkDefinition {
        id: ScheduledWorkId::new("schedule-sqlite").expect("id"),
        session_id: SessionId::new("session-sqlite").expect("session"),
        objective: "Run once".to_string(),
        route: route.clone(),
        execution_request: ScheduledWorkExecutionRequest {
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
        attention_policy: ScheduledWorkAttentionPolicy::AttentionOnly,
    };
    let occurrence = ScheduledWorkOccurrence {
        id: ScheduledWorkOccurrenceId::new("occurrence-sqlite").expect("id"),
        scheduled_work_id: definition.id.clone(),
        due_at_ms: 10,
        state: ScheduledWorkOccurrenceState::Pending,
    };
    let run = RunProjection {
        id: RunId::new("run-sqlite-scheduled").expect("run"),
        session_id: definition.session_id.clone(),
        runtime_profile_id: definition.route.runtime_profile_id.clone(),
        objective: definition.objective.clone(),
        status: RunStatus::Queued,
        harness: definition.route.harness,
        source: RunSource::ScheduledWork {
            route,
            scheduled_work_id: definition.id.clone(),
            occurrence_id: occurrence.id.clone(),
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
    (definition, occurrence, run)
}

static NEXT_STORE: AtomicU64 = AtomicU64::new(0);
fn with_store<T>(f: impl FnOnce(&mut SqliteStore) -> T) -> T {
    let label = format!(
        "scheduled-work-{}",
        NEXT_STORE.fetch_add(1, Ordering::Relaxed)
    );
    let path = test_db_path(&label);
    let mut store = SqliteStore::open(&path).expect("store");
    let result = f(&mut store);
    drop(store);
    let _ = std::fs::remove_file(path);
    result
}

fn terminal_transition(run: RunProjection) -> CommitRunTransition {
    let mut terminal = run;
    terminal.status = RunStatus::Completed;
    CommitRunTransition {
        session_id: terminal.session_id.clone(),
        run: terminal.clone(),
        user_turn: UserTurnCommit::NoUserTurn,
        events: vec![DaemonEvent::Run(
            RunEvent::terminal(
                terminal.id.clone(),
                RunStatus::Completed,
                RunStatusReason::new("done").expect("reason"),
                None,
                None,
                None,
            )
            .expect("event"),
        )],
        occurred_at_ms: 20,
        auth_profile_mutation: AuthProfileCommitMutation::Unchanged,
    }
}

#[test]
fn scheduled_work_current_schema_is_present_on_an_empty_store() {
    with_store(|store| {
        assert!(
            store
                .scheduled_work(&ScheduledWorkId::new("missing").expect("id"))
                .expect("query")
                .is_none()
        )
    });
}

#[test]
fn scheduled_work_claim_is_atomic_in_sqlite() {
    with_store(|store| {
        let (definition, occurrence, run) = fixture();
        store
            .save_session(session(definition.session_id.clone()))
            .expect("session");
        store
            .create_scheduled_work(definition.clone(), occurrence.clone())
            .expect("create");
        store
            .reserve_scheduled_work_occurrence(ReserveScheduledWorkOccurrence {
                scheduled_work_id: definition.id.clone(),
                occurrence_id: occurrence.id.clone(),
                run_id: run.id.clone(),
            })
            .expect("reserve");
        let claimed = store
            .publish_prepared_scheduled_work_occurrence(ClaimScheduledWorkOccurrence {
                scheduled_work_id: definition.id,
                occurrence_id: occurrence.id.clone(),
                run: run.clone(),
            })
            .expect("publish");
        assert_eq!(
            claimed.occurrence.state,
            ScheduledWorkOccurrenceState::Claimed {
                run_id: run.id.clone()
            }
        );
        assert_eq!(store.run(&run.id).expect("run"), Some(run));
    });
}

#[test]
fn scheduled_work_claim_rejects_every_frozen_payload_mismatch_in_sqlite() {
    for mutate in [
        |run: &mut RunProjection| run.objective.push('!'),
        |run: &mut RunProjection| run.source = crate::default_test_run_source(),
        |run: &mut RunProjection| {
            run.runtime_profile_id = RuntimeProfileId::new("runtime-other").expect("profile")
        },
        |run: &mut RunProjection| run.harness = RunHarnessKind::Acp,
        |run: &mut RunProjection| {
            run.session_id = SessionId::new("session-other").expect("session")
        },
        |run: &mut RunProjection| {
            let RunSource::ScheduledWork { route, .. } = &mut run.source else {
                unreachable!()
            };
            route.provider_id =
                ta_protocol::wire::AgentRuntimeStrategyId::new("provider-other").expect("provider");
        },
        |run: &mut RunProjection| {
            let RunSource::ScheduledWork { route, .. } = &mut run.source else {
                unreachable!()
            };
            route.model_id =
                Some(ta_protocol::wire::AgentRuntimeModelId::new("model-other").expect("model"));
        },
        |run: &mut RunProjection| {
            let RunSource::ScheduledWork { route, .. } = &mut run.source else {
                unreachable!()
            };
            route.auth_profile_id =
                Some(ta_protocol::wire::AuthProfileId::new("profile-other").expect("profile"));
        },
        |run: &mut RunProjection| {
            run.execution_context.permission_policy = ta_protocol::wire::PermissionPolicy::ReadOnly
        },
        |run: &mut RunProjection| {
            run.execution_context.workspace_id =
                ta_protocol::wire::WorkspaceId::new("workspace-other").expect("workspace")
        },
        |run: &mut RunProjection| {
            run.execution_context.workspace_scope = ta_protocol::wire::WorkspaceScope::Readonly {
                root: run.execution_context.workspace_root.clone(),
            }
        },
        |run: &mut RunProjection| {
            run.execution_context.sandbox_profile.process_exec =
                ta_protocol::wire::ProcessExecPolicy::Denied
        },
        |run: &mut RunProjection| {
            run.execution_context.network_policy = ta_protocol::wire::NetworkPolicy::None
        },
        |run: &mut RunProjection| {
            run.execution_context.env_policy = ta_protocol::wire::EnvPolicy::All
        },
    ] {
        with_store(|store| {
            let (definition, occurrence, mut run) = fixture();
            store
                .save_session(session(definition.session_id.clone()))
                .expect("session");
            store
                .create_scheduled_work(definition.clone(), occurrence.clone())
                .expect("create");
            mutate(&mut run);
            store
                .reserve_scheduled_work_occurrence(ReserveScheduledWorkOccurrence {
                    scheduled_work_id: definition.id.clone(),
                    occurrence_id: occurrence.id.clone(),
                    run_id: run.id.clone(),
                })
                .expect("reserve");
            assert!(matches!(
                store.publish_prepared_scheduled_work_occurrence(ClaimScheduledWorkOccurrence {
                    scheduled_work_id: definition.id,
                    occurrence_id: occurrence.id,
                    run
                }),
                Err(StoreError::ScheduledWorkRunSourceMismatch { .. })
            ));
        });
    }
}

#[test]
fn scheduled_work_terminal_settlement_is_atomic_in_sqlite() {
    with_store(|store| {
        let (definition, occurrence, run) = fixture();
        store
            .save_session(session(definition.session_id.clone()))
            .expect("session");
        store
            .create_scheduled_work(definition.clone(), occurrence.clone())
            .expect("create");
        store
            .reserve_scheduled_work_occurrence(ReserveScheduledWorkOccurrence {
                scheduled_work_id: definition.id.clone(),
                occurrence_id: occurrence.id.clone(),
                run_id: run.id.clone(),
            })
            .expect("reserve");
        store
            .publish_prepared_scheduled_work_occurrence(ClaimScheduledWorkOccurrence {
                scheduled_work_id: definition.id,
                occurrence_id: occurrence.id.clone(),
                run: run.clone(),
            })
            .expect("publish");
        store
            .commit_run_transition(terminal_transition(run.clone()))
            .expect("terminal");
        assert_eq!(
            store
                .scheduled_work_occurrence(&occurrence.id)
                .expect("occurrence")
                .expect("stored")
                .state,
            ScheduledWorkOccurrenceState::Completed { run_id: run.id }
        );
    });
}

#[test]
fn scheduled_work_terminal_rejects_occurrence_not_claimed_by_exact_run_in_sqlite() {
    with_store(|store| {
        let (definition, occurrence, run) = fixture();
        store
            .save_session(session(definition.session_id.clone()))
            .expect("session");
        store
            .create_scheduled_work(definition.clone(), occurrence.clone())
            .expect("create");
        let mut other = run.clone();
        other.id = RunId::new("run-sqlite-other").expect("run");
        store
            .reserve_scheduled_work_occurrence(ReserveScheduledWorkOccurrence {
                scheduled_work_id: definition.id.clone(),
                occurrence_id: occurrence.id.clone(),
                run_id: other.id.clone(),
            })
            .expect("reserve");
        store
            .publish_prepared_scheduled_work_occurrence(ClaimScheduledWorkOccurrence {
                scheduled_work_id: definition.id,
                occurrence_id: occurrence.id,
                run: other,
            })
            .expect("publish");
        store.save_run(run.clone()).expect("seed run");
        assert!(matches!(
            store.commit_run_transition(terminal_transition(run)),
            Err(StoreError::ScheduledWorkOccurrenceClaimMismatch { .. })
        ));
    });
}

#[test]
fn scheduled_work_cancellation_intent_wins_publication_and_cleanup_terminal_in_sqlite() {
    with_store(|store| {
        let (definition, occurrence, run) = fixture();
        store
            .save_session(session(definition.session_id.clone()))
            .expect("session");
        store
            .create_scheduled_work(definition.clone(), occurrence.clone())
            .expect("create");
        store
            .reserve_scheduled_work_occurrence(ReserveScheduledWorkOccurrence {
                scheduled_work_id: definition.id.clone(),
                occurrence_id: occurrence.id.clone(),
                run_id: run.id.clone(),
            })
            .expect("reserve");
        let resource = ta_protocol::wire::ScheduledWorkUnpublishedResource {
            parent_repo: "/repo".to_string(),
            worktree_path: "/repo/target/taugentic-worktrees/run-sqlite-scheduled".to_string(),
            branch: "ta/capsule-run-sqlite-scheduled".to_string(),
            cleanup_policy: ta_protocol::wire::WorktreeCleanupPolicy::DeleteOnSuccess,
        };
        store
            .request_preparing_scheduled_work_cancellation(
                &occurrence.id,
                &run.id,
                resource.clone(),
            )
            .expect("durable cancellation");
        assert!(matches!(
            store.publish_prepared_scheduled_work_occurrence(ClaimScheduledWorkOccurrence {
                scheduled_work_id: definition.id,
                occurrence_id: occurrence.id.clone(),
                run: run.clone()
            }),
            Err(StoreError::ScheduledWorkOccurrenceNotPending { .. })
        ));
        let stored = store
            .finalize_preparing_scheduled_work_cleanup(
                &occurrence.id,
                &run.id,
                ta_protocol::wire::ScheduledWorkPreparationTerminal::Failed,
                ta_protocol::wire::ScheduledWorkUnpublishedResource {
                    parent_repo: "wrong".to_string(),
                    worktree_path: "wrong".to_string(),
                    branch: "wrong".to_string(),
                    cleanup_policy: ta_protocol::wire::WorktreeCleanupPolicy::Manual,
                },
                "publish lost cancellation race".to_string(),
                Err("cleanup failed".to_string()),
            )
            .expect("cancelled cleanup terminal");
        assert_eq!(
            stored.state,
            ScheduledWorkOccurrenceState::CleanupRequired {
                run_id: run.id.clone(),
                resource,
                intended_terminal: ta_protocol::wire::ScheduledWorkPreparationTerminal::Cancelled,
                preparation_detail: "publish lost cancellation race".to_string(),
                cleanup_detail: "cleanup failed".to_string()
            }
        );
        assert!(store.run(&run.id).expect("run lookup").is_none());
    });
}
