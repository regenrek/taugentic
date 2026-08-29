use super::*;
use crate::{
    AuthProfileCommitMutation, ClaimScheduledWorkOccurrence, CommitRepository, CommitRunTransition,
    ReserveScheduledWorkOccurrence, ScheduledWorkRepository, StoreSeedRepository, UserTurnCommit,
};
use ta_protocol::wire::{
    DaemonEvent, RunEvent, RunHarnessKind, RunId, RunSource, RunStatus, RunStatusReason,
    RuntimeProfileId, ScheduledWorkAttentionPolicy, ScheduledWorkDefinition,
    ScheduledWorkExecutionRequest, ScheduledWorkId, ScheduledWorkOccurrence,
    ScheduledWorkOccurrenceId, ScheduledWorkOccurrenceState, SessionId, SessionNextRunSelection,
    SessionStatus,
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
        id: ScheduledWorkId::new("schedule-memory").expect("id"),
        session_id: SessionId::new("session-memory").expect("session"),
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
        id: ScheduledWorkOccurrenceId::new("occurrence-memory").expect("id"),
        scheduled_work_id: definition.id.clone(),
        due_at_ms: 10,
        state: ScheduledWorkOccurrenceState::Pending,
    };
    let run = RunProjection {
        id: RunId::new("run-memory-scheduled").expect("run id"),
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

fn seeded_claim(
    mutator: impl FnOnce(&mut RunProjection),
) -> (InMemoryStore, ScheduledWorkOccurrence, RunProjection) {
    let mut store = InMemoryStore::current();
    let (definition, occurrence, mut run) = fixture();
    store
        .save_session(session(definition.session_id.clone()))
        .expect("session");
    store
        .create_scheduled_work(definition.clone(), occurrence.clone())
        .expect("create");
    mutator(&mut run);
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
            run,
        })
        .expect("publish");
    (store, occurrence, claimed.run)
}

#[test]
fn scheduled_work_claim_is_one_memory_mutation_with_queued_run() {
    let (store, occurrence, run) = seeded_claim(|_| {});
    assert_eq!(
        store
            .scheduled_work_occurrence(&occurrence.id)
            .expect("occurrence")
            .expect("stored")
            .state,
        ScheduledWorkOccurrenceState::Claimed {
            run_id: run.id.clone()
        }
    );
    assert_eq!(store.run(&run.id).expect("run lookup"), Some(run));
}

#[test]
fn scheduled_work_claim_rejects_every_frozen_payload_mismatch() {
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
        let mut store = InMemoryStore::current();
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
                occurrence_id: occurrence.id.clone(),
                run
            }),
            Err(StoreError::ScheduledWorkRunSourceMismatch { .. })
        ));
        assert!(
            store
                .run(&RunId::new("run-memory-scheduled").expect("run"))
                .expect("run query")
                .is_none()
        );
        assert_eq!(
            store
                .scheduled_work_occurrence(&occurrence.id)
                .expect("occurrence")
                .expect("stored")
                .state,
            ScheduledWorkOccurrenceState::Preparing {
                run_id: RunId::new("run-memory-scheduled").expect("run")
            }
        );
    }
}

fn terminal_transition(run: RunProjection) -> CommitRunTransition {
    let mut terminal = run.clone();
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
fn scheduled_work_terminal_settlement_is_atomic_in_memory() {
    let (mut store, occurrence, run) = seeded_claim(|_| {});
    store
        .commit_run_transition(terminal_transition(run.clone()))
        .expect("terminal commit");
    assert_eq!(
        store.run(&run.id).expect("run").expect("stored").status,
        RunStatus::Completed
    );
    assert_eq!(
        store
            .scheduled_work_occurrence(&occurrence.id)
            .expect("occurrence")
            .expect("stored")
            .state,
        ScheduledWorkOccurrenceState::Completed { run_id: run.id }
    );
}

#[test]
fn scheduled_work_terminal_rejects_occurrence_not_claimed_by_exact_run_in_memory() {
    let mut store = InMemoryStore::current();
    let (definition, occurrence, run) = fixture();
    store
        .save_session(session(definition.session_id.clone()))
        .expect("session");
    store
        .create_scheduled_work(definition.clone(), occurrence.clone())
        .expect("create");
    let mut other = run.clone();
    other.id = RunId::new("run-memory-other").expect("run");
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
        .expect("other claim");
    store.save_run(run.clone()).expect("seed run");
    assert!(matches!(
        store.commit_run_transition(terminal_transition(run)),
        Err(StoreError::ScheduledWorkOccurrenceClaimMismatch { .. })
    ));
}

#[test]
fn scheduled_work_preparation_cleanup_failure_retains_exact_resource_in_memory() {
    let mut store = InMemoryStore::current();
    let (definition, occurrence, run) = fixture();
    store
        .create_scheduled_work(definition.clone(), occurrence.clone())
        .expect("create");
    store
        .reserve_scheduled_work_occurrence(ReserveScheduledWorkOccurrence {
            scheduled_work_id: definition.id,
            occurrence_id: occurrence.id.clone(),
            run_id: run.id.clone(),
        })
        .expect("reserve");
    let resource = ta_protocol::wire::ScheduledWorkUnpublishedResource {
        parent_repo: "/repo".to_string(),
        worktree_path: "/repo/target/taugentic-worktrees/run-memory-scheduled".to_string(),
        branch: "ta/capsule-run-memory-scheduled".to_string(),
        cleanup_policy: ta_protocol::wire::WorktreeCleanupPolicy::DeleteOnSuccess,
    };
    let stored = store
        .finalize_preparing_scheduled_work_cleanup(
            &occurrence.id,
            &run.id,
            ta_protocol::wire::ScheduledWorkPreparationTerminal::Failed,
            resource.clone(),
            "prepare failed".to_string(),
            Err("git removal failed".to_string()),
        )
        .expect("terminalizes visibly");
    assert_eq!(
        stored.state,
        ScheduledWorkOccurrenceState::CleanupRequired {
            run_id: run.id,
            resource,
            intended_terminal: ta_protocol::wire::ScheduledWorkPreparationTerminal::Failed,
            preparation_detail: "prepare failed".to_string(),
            cleanup_detail: "git removal failed".to_string()
        }
    );
}

#[test]
fn scheduled_work_cancellation_intent_wins_publication_and_cleanup_terminal_in_memory() {
    let mut store = InMemoryStore::current();
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
        worktree_path: "/repo/target/taugentic-worktrees/run-memory-scheduled".to_string(),
        branch: "ta/capsule-run-memory-scheduled".to_string(),
        cleanup_policy: ta_protocol::wire::WorktreeCleanupPolicy::DeleteOnSuccess,
    };
    store
        .request_preparing_scheduled_work_cancellation(&occurrence.id, &run.id, resource.clone())
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
}
