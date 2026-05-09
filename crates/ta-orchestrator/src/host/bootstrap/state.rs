use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use ta_protocol::wire::{
    ApprovalActor, ApprovalDecision, ApprovalEvent, ApprovalResolution, ApprovalResolutionReason,
    DaemonEvent, RunFailureKind, RunReconciledOnStartupEvent, RunStatus,
};
use ta_store::{
    CommitRunTransition, CommitStartupReconciliation, PersistenceStore, SessionApprovalQuery,
    SqliteStore, StoreError,
};
use thiserror::Error;

use crate::{
    AppService, RecipeRegistry, RecipeRegistryError, RuntimeExecutionPaths, RuntimeService,
    host::config::DaemonConfig,
};

const RESTART_RECONCILE_DETAIL: &str = "daemon restarted while run was active";

pub struct BootstrapState<S = SqliteStore>
where
    S: PersistenceStore + Send + 'static,
{
    pub config: DaemonConfig,
    pub runtime: RuntimeService,
    pub app: AppService<S>,
    pub recipe_registry: Arc<RecipeRegistry>,
    started_at_ms: u64,
    in_flight_rpc_count: Arc<AtomicUsize>,
}

impl<S> Clone for BootstrapState<S>
where
    S: PersistenceStore + Send + 'static,
{
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            runtime: self.runtime.clone(),
            app: self.app.clone(),
            recipe_registry: Arc::clone(&self.recipe_registry),
            started_at_ms: self.started_at_ms,
            in_flight_rpc_count: Arc::clone(&self.in_flight_rpc_count),
        }
    }
}

#[derive(Debug, Error)]
pub enum BootstrapStateError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    RecipeRegistry(#[from] RecipeRegistryError),
}

pub fn open_bootstrap_state(config: DaemonConfig) -> Result<BootstrapState, BootstrapStateError> {
    let store = SqliteStore::open(config.store_path())?;
    boot_with_store(config, Arc::new(Mutex::new(store)))
}

#[cfg(test)]
pub fn boot(config: DaemonConfig) -> BootstrapState {
    let state = open_bootstrap_state(config).expect("daemon store should open");
    state
        .app
        .upsert_workspace(ta_store::default_test_workspace())
        .expect("seed default test workspace");
    state
}

pub fn boot_with_store<S>(
    config: DaemonConfig,
    store: Arc<Mutex<S>>,
) -> Result<BootstrapState<S>, BootstrapStateError>
where
    S: PersistenceStore + Send + 'static,
{
    reconcile_orphaned_running_runs(&store)?;
    let recipe_registry_outcome = RecipeRegistry::load_with_user_dir(
        ta_host_platform::taugentic_user_recipe_dir().as_deref(),
    )?;
    for diagnostic in &recipe_registry_outcome.diagnostics {
        tracing::warn!(
            path = %diagnostic.path.display(),
            error = %diagnostic.error,
            "user recipe failed to load, skipping"
        );
    }
    let recipe_registry = Arc::new(recipe_registry_outcome.registry);
    let runtime = RuntimeService::from_host_platform_with_paths(
        ta_host_platform::detect_current_platform(),
        RuntimeExecutionPaths {
            working_directory: std::env::current_dir()
                .expect("daemon working directory should be available"),
            artifact_root: config.artifact_root(),
        },
    );
    let app = AppService::from_runtime_with_recipes(store, &runtime, Arc::clone(&recipe_registry));
    load_default_workflow_if_present(&app);
    app.rehydrate_run_scheduler_on_boot()
        .map_err(|error| match error {
            crate::orchestration::AppServiceError::Store(error) => error,
            other => StoreError::ApprovalLifecycleViolation {
                approval_id: "run-scheduler-bootstrap".to_string(),
                detail: other.to_string(),
            },
        })?;
    Ok(BootstrapState {
        config,
        runtime,
        app,
        recipe_registry,
        started_at_ms: current_time_ms(),
        in_flight_rpc_count: Arc::new(AtomicUsize::new(0)),
    })
}

fn load_default_workflow_if_present<S>(app: &AppService<S>)
where
    S: PersistenceStore + Send + 'static,
{
    let Some(path) = ta_host_platform::taugentic_workflow_file_path() else {
        return;
    };
    if !path.exists() {
        return;
    }
    match app.load_workflow(&crate::WorkflowLoadParams {
        path: path.display().to_string(),
    }) {
        Ok(status) if status.loaded.is_some() => {
            tracing::info!(path = %path.display(), "loaded default workflow file");
        }
        Ok(status) => {
            tracing::warn!(
                path = %path.display(),
                last_reload = ?status.last_reload,
                "default workflow file is invalid; background orchestrator remains idle"
            );
        }
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "failed to load default workflow file; background orchestrator remains idle"
            );
        }
    }
}

impl<S> BootstrapState<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(crate) fn uptime_ms(&self) -> u64 {
        current_time_ms().saturating_sub(self.started_at_ms)
    }

    pub(crate) fn in_flight_rpc_count(&self) -> usize {
        self.in_flight_rpc_count.load(Ordering::SeqCst)
    }

    pub(crate) fn track_rpc_request(&self) -> InFlightRpcGuard {
        self.in_flight_rpc_count.fetch_add(1, Ordering::SeqCst);
        InFlightRpcGuard {
            counter: Arc::clone(&self.in_flight_rpc_count),
        }
    }
}

pub(crate) struct InFlightRpcGuard {
    counter: Arc<AtomicUsize>,
}

impl Drop for InFlightRpcGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

fn reconcile_orphaned_running_runs<S>(store: &Arc<Mutex<S>>) -> Result<(), StoreError>
where
    S: PersistenceStore + Send,
{
    let active_runs = {
        let store = store.lock().expect("app store should not be poisoned");
        store
            .runs()?
            .into_iter()
            .filter(is_active_status)
            .collect::<Vec<_>>()
    };

    if active_runs.is_empty() {
        return Ok(());
    }

    let occurred_at_ms = current_time_ms();
    let mut store = store.lock().expect("app store should not be poisoned");
    let mut transitions = Vec::with_capacity(active_runs.len());
    for run in active_runs {
        let prev_status = run.status;
        let age_ms = run
            .started_at_ms
            .map(|started_at_ms| occurred_at_ms.saturating_sub(started_at_ms))
            .unwrap_or(0);
        tracing::warn!(
            run_id = run.id.as_str(),
            prev_status = ?prev_status,
            age_ms,
            "reconciled active run after daemon restart"
        );
        let mut events = vec![DaemonEvent::Run(crate::RunEvent {
            run_id: run.id.clone(),
            status: RunStatus::Failed,
            detail: RESTART_RECONCILE_DETAIL.to_string(),
            output_contract: None,
            recipe_id: None,
            result: None,
        })];
        events.extend(
            store
                .approvals_for_session(&SessionApprovalQuery {
                    session_id: run.session_id.clone(),
                    run_id: Some(run.id.clone()),
                    approval_id: None,
                })?
                .into_iter()
                .map(|approval| {
                    let mut resolution = ApprovalResolution::new(
                        approval.id,
                        approval.run_id,
                        ApprovalDecision::Rejected,
                        ApprovalResolutionReason::Cancelled,
                        daemon_startup_actor(),
                        Some("daemon_restarted_while_run_was_active".to_string()),
                    );
                    resolution.tool_call_id = approval.tool_call_id;
                    DaemonEvent::Approval(ApprovalEvent::Resolved { resolution })
                }),
        );
        events.push(DaemonEvent::RunReconciledOnStartup(
            RunReconciledOnStartupEvent {
                run_id: run.id.clone(),
                prev_status,
                reason: RunFailureKind::DaemonRestartedWhileRunning,
            },
        ));
        transitions.push(CommitRunTransition {
            session_id: run.session_id.clone(),
            run: ta_store::RunProjection {
                status: RunStatus::Failed,
                ..run.clone()
            },
            events,
            occurred_at_ms,
        });
    }
    store.commit_startup_reconciliation(CommitStartupReconciliation { transitions })?;

    Ok(())
}

fn daemon_startup_actor() -> ApprovalActor {
    ApprovalActor::new("taugentic-daemon").expect("daemon actor id should be valid")
}

fn is_active_status(run: &ta_store::RunProjection) -> bool {
    matches!(
        run.status,
        RunStatus::Running | RunStatus::WaitingForApproval
    )
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActivityPageQuery, DaemonApprovalDecideParams, DaemonEventKind, LaneCapabilities,
        ListSessionsQuery, OpenSessionRequest, RunStatus, SessionStatus, StartRunCommand,
    };
    use ta_protocol::wire::ApprovalDecision;
    use ta_store::{CheckpointRecord, CommitCheckpointPersist, CommitRepository};

    const TEST_OWNER_PRINCIPAL_ID: &str = "bootstrap-owner-credential-hash";
    const TEST_CLIENT_NAME: &str = "bootstrap-tests";

    #[test]
    fn boot_uses_runtime_owned_capability_derivation() {
        crate::host::config::with_test_config_home("boot-runtime-capabilities", || {
            let config = crate::host::config::test_config();
            let state = boot(config.clone());

            assert_eq!(state.config, config);
            assert_eq!(
                state.runtime.capabilities().clone(),
                LaneCapabilities::from_host_platform(&state.runtime.host_platform)
            );
        });
    }

    #[test]
    fn boot_reloads_persisted_sessions_from_store_snapshot() {
        crate::host::config::with_test_config_home("boot-reloads-store-snapshot", || {
            let config = crate::host::config::test_config();
            let first = open_bootstrap_state(config.clone()).expect("first boot should succeed");
            first
                .app
                .upsert_workspace(ta_store::default_test_workspace())
                .expect("seed workspace");
            let created = first
                .app
                .open_session(
                    TEST_CLIENT_NAME,
                    TEST_OWNER_PRINCIPAL_ID,
                    &OpenSessionRequest {
                        title: "Persist me".to_string(),
                        workspace_id: ta_store::default_test_workspace_id(),
                    },
                )
                .expect("session should persist");

            let second = open_bootstrap_state(config).expect("second boot should succeed");
            let sessions = second
                .app
                .list_sessions(
                    TEST_CLIENT_NAME,
                    TEST_OWNER_PRINCIPAL_ID,
                    &crate::ListSessionsQuery {},
                )
                .expect("sessions should load");

            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].id, created.id);
            assert_eq!(sessions[0].title, "Persist me");
        });
    }

    #[test]
    fn boot_reloads_committed_runs_and_session_status_from_store() {
        crate::host::config::with_test_config_home("boot-reloads-committed-run", || {
            let config = crate::host::config::test_config();
            let first = open_bootstrap_state(config.clone()).expect("first boot should succeed");
            first
                .app
                .upsert_workspace(ta_store::default_test_workspace())
                .expect("seed workspace");
            let session = first
                .app
                .open_session(
                    TEST_CLIENT_NAME,
                    TEST_OWNER_PRINCIPAL_ID,
                    &OpenSessionRequest {
                        title: "Persist me".to_string(),
                        workspace_id: ta_store::default_test_workspace_id(),
                    },
                )
                .expect("session should persist");
            let started = first
                .app
                .start_run(
                    &session.id,
                    &StartRunCommand {
                        objective: "Ship store boundary".to_string(),
                        ..StartRunCommand::default()
                    },
                )
                .expect("run should start");
            match started.body.status {
                RunStatus::WaitingForApproval => {
                    let approval_id = started
                        .requested_approval_id()
                        .expect("waiting run should request approval");
                    first
                        .app
                        .decide_approval(
                            &session.id,
                            &ta_protocol::wire::ApprovalActor::new(TEST_OWNER_PRINCIPAL_ID)
                                .expect("approval actor"),
                            &DaemonApprovalDecideParams {
                                approval_id,
                                decision: ApprovalDecision::Rejected,
                                commentary: Some("keep reboot proof non-live".to_string()),
                            },
                        )
                        .expect("approval rejection should persist");
                }
                RunStatus::Running => {
                    first
                        .app
                        .cancel_run(
                            &session.id,
                            &ta_protocol::wire::ApprovalActor::new(TEST_OWNER_PRINCIPAL_ID)
                                .expect("approval actor"),
                            &started.body.id,
                            Some("keep reboot proof non-live".to_string()),
                        )
                        .expect("running run should cancel before reboot");
                }
                status => panic!("unexpected start status for reboot proof: {status:?}"),
            }

            let second = open_bootstrap_state(config).expect("second boot should succeed");
            let sessions = second
                .app
                .list_sessions(
                    TEST_CLIENT_NAME,
                    TEST_OWNER_PRINCIPAL_ID,
                    &crate::ListSessionsQuery {},
                )
                .expect("sessions should load");
            let runs = second.app.list_runs(&session.id).expect("runs should load");

            assert_eq!(sessions.len(), 1);
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].id, started.body.id);
            let expected_session_status = match runs[0].status {
                RunStatus::Failed => SessionStatus::Failed,
                RunStatus::Cancelled => SessionStatus::Idle,
                status => panic!("unexpected persisted run status after reboot: {status:?}"),
            };
            assert_eq!(sessions[0].status, expected_session_status);
        });
    }

    #[test]
    fn boot_reconciles_active_running_runs_without_live_owner_after_restart() {
        crate::host::config::with_test_config_home("boot-reconciles-running-runs", || {
            let config = crate::host::config::test_config();
            let first = open_bootstrap_state(config.clone()).expect("first boot should succeed");
            first
                .app
                .upsert_workspace(ta_store::default_test_workspace())
                .expect("seed workspace");
            let session = first
                .app
                .open_session(
                    TEST_CLIENT_NAME,
                    TEST_OWNER_PRINCIPAL_ID,
                    &OpenSessionRequest {
                        title: "Reconcile me".to_string(),
                        workspace_id: ta_store::default_test_workspace_id(),
                    },
                )
                .expect("session should persist");
            let started = first
                .app
                .seed_running_run_for_tests(&session.id, "Become durable running")
                .expect("seeded run should persist");
            let running_run_id = started.body.id.clone();

            let second = open_bootstrap_state(config).expect("second boot should succeed");
            let sessions = second
                .app
                .list_sessions(
                    TEST_CLIENT_NAME,
                    TEST_OWNER_PRINCIPAL_ID,
                    &ListSessionsQuery {},
                )
                .expect("sessions should load");
            let runs = second.app.list_runs(&session.id).expect("runs should load");
            let activity = second
                .app
                .activity_page(
                    &session.id,
                    &ActivityPageQuery {
                        limit: 20,
                        before: None,
                        kinds: vec![DaemonEventKind::Run],
                    },
                )
                .expect("activity page should load");

            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].status, SessionStatus::Failed);
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].id, running_run_id);
            assert_eq!(runs[0].status, RunStatus::Failed);
            assert!(activity.items.iter().any(|item| {
                matches!(
                    &item.event,
                    crate::PublicDaemonEvent::Run(crate::RunEvent {
                        run_id,
                        status,
                        detail,
                        ..
                    })
                        if *run_id == running_run_id
                            && *status == RunStatus::Failed
                            && detail == RESTART_RECONCILE_DETAIL
                )
            }));
        });
    }

    #[test]
    fn boot_reconciles_waiting_for_approval_runs_after_restart() {
        crate::host::config::with_test_config_home("boot-reconciles-waiting-approvals", || {
            let config = crate::host::config::test_config();
            let first = open_bootstrap_state(config.clone()).expect("first boot should succeed");
            first
                .app
                .upsert_workspace(ta_store::default_test_workspace())
                .expect("seed workspace");
            let session = first
                .app
                .open_session(
                    TEST_CLIENT_NAME,
                    TEST_OWNER_PRINCIPAL_ID,
                    &OpenSessionRequest {
                        title: "Preserve pending approval".to_string(),
                        workspace_id: ta_store::default_test_workspace_id(),
                    },
                )
                .expect("session should persist");
            let started = first
                .app
                .start_run(
                    &session.id,
                    &StartRunCommand {
                        objective: "Need approval after restart".to_string(),
                        ..StartRunCommand::default()
                    },
                )
                .expect("run should start");

            let approval_id = match started.body.status {
                RunStatus::WaitingForApproval => started
                    .requested_approval_id()
                    .expect("waiting run should request approval"),
                RunStatus::Running => {
                    return;
                }
                status => {
                    panic!("unexpected start status for approval persistence proof: {status:?}")
                }
            };

            let second = open_bootstrap_state(config).expect("second boot should succeed");
            let runs = second.app.list_runs(&session.id).expect("runs should load");
            let approvals = second
                .app
                .list_approvals(
                    &session.id,
                    &crate::ListApprovalsQuery {
                        run_id: Some(started.body.id.clone()),
                        approval_id: Some(approval_id.clone()),
                    },
                )
                .expect("approvals should load");

            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0].id, started.body.id);
            assert_eq!(runs[0].status, RunStatus::Failed);
            assert!(approvals.items.is_empty());
            assert!(approvals.latest_cursor.is_some());
        });
    }

    #[test]
    fn boot_promotes_the_oldest_queued_run_after_reconciling_running_owner_loss() {
        crate::host::config::with_test_config_home("boot-promotes-queued-run", || {
            let config = crate::host::config::test_config();
            let first = open_bootstrap_state(config.clone()).expect("first boot should succeed");
            first
                .app
                .upsert_workspace(ta_store::default_test_workspace())
                .expect("seed workspace");
            let session = first
                .app
                .open_session(
                    TEST_CLIENT_NAME,
                    TEST_OWNER_PRINCIPAL_ID,
                    &OpenSessionRequest {
                        title: "Promote queued run".to_string(),
                        workspace_id: ta_store::default_test_workspace_id(),
                    },
                )
                .expect("session should persist");
            let active = first
                .app
                .seed_running_run_for_tests(&session.id, "Occupy active slot")
                .expect("seeded active run should persist");
            let active_run_id = active.body.id.clone();
            first
                .runtime
                .run_execution_runtime()
                .schedule_run_start(&session.id, active_run_id.clone())
                .expect("seeded active run should occupy scheduler");
            let queued = first
                .app
                .start_run(
                    &session.id,
                    &StartRunCommand {
                        objective: "Queued behind active".to_string(),
                        ..StartRunCommand::default()
                    },
                )
                .expect("second run should queue");

            assert_eq!(queued.body.status, RunStatus::Queued);

            let second = open_bootstrap_state(config).expect("second boot should succeed");
            let runs = second.app.list_runs(&session.id).expect("runs should load");
            let failed_active = runs
                .iter()
                .find(|run| run.id == active_run_id)
                .expect("active run should remain durable");
            let promoted = runs
                .iter()
                .find(|run| run.id == queued.body.id)
                .expect("queued run should remain durable");

            assert_eq!(failed_active.status, RunStatus::Failed);
            assert!(matches!(
                promoted.status,
                RunStatus::Running | RunStatus::WaitingForApproval
            ));
        });
    }

    #[test]
    fn boot_fails_checkpointed_running_run_after_restart() {
        crate::host::config::with_test_config_home("boot-fails-checkpointed-running-run", || {
            let config = crate::host::config::test_config();
            let first = open_bootstrap_state(config.clone()).expect("first boot should succeed");
            first
                .app
                .upsert_workspace(ta_store::default_test_workspace())
                .expect("seed workspace");
            let session = first
                .app
                .open_session(
                    TEST_CLIENT_NAME,
                    TEST_OWNER_PRINCIPAL_ID,
                    &OpenSessionRequest {
                        title: "Resume checkpointed run".to_string(),
                        workspace_id: ta_store::default_test_workspace_id(),
                    },
                )
                .expect("session should persist");
            let started = first
                .app
                .seed_running_run_for_tests(&session.id, "Checkpoint before restart")
                .expect("seeded run should persist");
            let running_run_id = started.body.id.clone();

            SqliteStore::open(config.store_path())
                .expect("sqlite store should reopen")
                .commit_checkpoint_persist(CommitCheckpointPersist {
                    checkpoint: CheckpointRecord {
                        run_id: running_run_id.clone(),
                        revision: 1,
                        artifact_path: format!(
                            "checkpoints/{}/rev-1.json",
                            running_run_id.as_str()
                        ),
                    },
                    occurred_at_ms: 42,
                })
                .expect("checkpoint should persist");

            let second = open_bootstrap_state(config).expect("second boot should succeed");
            let runs = second.app.list_runs(&session.id).expect("runs should load");
            let reconciled = runs
                .iter()
                .find(|run| run.id == running_run_id)
                .expect("checkpointed run should remain durable");
            let activity = second
                .app
                .activity_page(
                    &session.id,
                    &ActivityPageQuery {
                        limit: 20,
                        before: None,
                        kinds: vec![DaemonEventKind::Run],
                    },
                )
                .expect("activity page should load");

            assert_eq!(reconciled.status, RunStatus::Failed);
            assert!(activity.items.iter().any(|item| {
                matches!(
                    &item.event,
                    crate::PublicDaemonEvent::Run(crate::RunEvent {
                        run_id,
                        status,
                        detail,
                        ..
                    })
                        if *run_id == running_run_id
                            && *status == RunStatus::Failed
                            && detail == RESTART_RECONCILE_DETAIL
                )
            }));
        });
    }
}
