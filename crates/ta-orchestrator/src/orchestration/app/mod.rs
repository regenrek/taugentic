use std::collections::HashSet;
use std::ops::Deref;
use std::sync::{Arc, Mutex, atomic::AtomicBool};

use ta_store::{InMemoryStore, PersistenceStore};
use ta_workflow::WorkflowManager;

use crate::orchestration::{AgentRuntimeService, RunExecutionService};
use crate::{RecipeRegistry, RuntimeService, SessionAuthority, SessionSummary, WorkItemKey};

mod agent_runtime;
mod approvals;
mod code_host;
mod diagnostics;
mod errors;
mod git;
mod native_runs;
mod navigation;
mod plugins;
mod projections;
mod queries;
mod receipts;
mod runs;
mod scheduled_work;
mod sessions;
mod terminals;
mod thread_workspace;
mod timeline;
mod work_item_poller;
mod work_items;
mod workflow;
mod workspace_files;
mod workspaces;

#[cfg(test)]
mod tests;

pub use errors::AppServiceError;
pub(crate) use errors::recipe_resolution_error_data;

use errors::{map_artifact_mutation_result, map_run_execution_error, map_run_mutation_result};
use projections::{
    clamp_session_overview_recent_activity_limit, index_run_summaries_by_session,
    project_latest_run_for_session, project_run_detail, project_run_list_entry,
    project_session_overview_lane_status, project_session_summary,
    session_overview_recent_activity_kinds, summarize_event_preview,
};
use sessions::{sanitize_session_owner_client_name, sanitize_session_owner_principal_id};

pub struct AppService<S = InMemoryStore>
where
    S: PersistenceStore + Send + 'static,
{
    pub(super) daemon_instance_id: String,
    pub(super) runtime: RuntimeService,
    pub(super) store: Arc<Mutex<S>>,
    pub(super) recipe_registry: Arc<RecipeRegistry>,
    pub(super) work_source_refresh_requested: Arc<AtomicBool>,
    pub(super) work_item_trigger_flights: Arc<Mutex<HashSet<WorkItemKey>>>,
    pub(super) workflow: WorkflowManager,
    pub(super) agent_runtime: AgentRuntimeService<S>,
    pub(super) run_execution: RunExecutionService<S>,
    pub(super) git_reverts: git::GitRevertRuntime,
    pub(super) code_host_pushes: code_host::CodeHostPushRuntime,
}

impl<S> Clone for AppService<S>
where
    S: PersistenceStore + Send + 'static,
{
    fn clone(&self) -> Self {
        Self {
            daemon_instance_id: self.daemon_instance_id.clone(),
            runtime: self.runtime.clone(),
            store: Arc::clone(&self.store),
            recipe_registry: Arc::clone(&self.recipe_registry),
            work_source_refresh_requested: Arc::clone(&self.work_source_refresh_requested),
            work_item_trigger_flights: Arc::clone(&self.work_item_trigger_flights),
            workflow: self.workflow.clone(),
            agent_runtime: self.agent_runtime.clone(),
            run_execution: self.run_execution.clone(),
            git_reverts: self.git_reverts.clone(),
            code_host_pushes: self.code_host_pushes.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSessionRequest {
    pub title: String,
    pub workspace_id: ta_protocol::wire::WorkspaceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenWorkspaceRequest {
    pub path: ta_protocol::wire::WorkspacePath,
    pub trust_acknowledged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPrincipalResolution {
    pub client_name: String,
    pub principal_id: String,
    pub client_credential: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSessionResult {
    pub session: SessionSummary,
    pub session_authority: SessionAuthority,
}

impl Deref for OpenSessionResult {
    type Target = SessionSummary;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachSessionResult {
    pub session: SessionSummary,
    pub session_authority: SessionAuthority,
}

impl Deref for AttachSessionResult {
    type Target = SessionSummary;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDeferredMutationResult<T> {
    pub body: T,
    pub deferred_records: Vec<ta_store::EventRecord>,
}

impl<T> AppDeferredMutationResult<T> {
    #[cfg(test)]
    pub(crate) fn requested_approval_id(&self) -> Option<ta_protocol::wire::ApprovalId> {
        self.deferred_records
            .iter()
            .find_map(|record| match &record.payload {
                crate::DaemonEvent::Approval(crate::ApprovalEvent::Requested { request }) => {
                    Some(request.id.clone())
                }
                _ => None,
            })
    }
}

impl AppService<InMemoryStore> {
    #[cfg(any(test, feature = "test-support"))]
    pub fn bootstrap() -> Result<Self, AppServiceError> {
        let service = Self::bootstrap_with_runtime(RuntimeService::bootstrap())?;
        // Seed the canonical test workspace so test fixtures that call
        // `open_session` with `ta_store::default_test_workspace_id()` find
        // a valid FK without each test having to bootstrap the workspace
        // explicitly. Production deployments do not invoke `bootstrap`.
        service.upsert_workspace(ta_store::default_test_workspace())?;
        Ok(service)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn bootstrap_with_runtime(runtime: RuntimeService) -> Result<Self, AppServiceError> {
        Ok(Self::from_runtime(
            Arc::new(Mutex::new(InMemoryStore::current())),
            &runtime,
        ))
    }
}

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn seed_auth_profile_for_tests(
        &self,
        profile: ta_store::AuthProfileProjection,
    ) -> Result<(), AppServiceError> {
        self.store
            .lock()
            .expect("app store should not be poisoned")
            .save_auth_profile(profile)?;
        Ok(())
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn from_runtime(store: Arc<Mutex<S>>, runtime: &RuntimeService) -> Self {
        let recipe_registry =
            Arc::new(RecipeRegistry::load_builtin().expect("built-in recipes should load"));
        Self::from_runtime_with_recipes(store, runtime, recipe_registry)
    }

    pub(crate) fn from_runtime_with_recipes(
        store: Arc<Mutex<S>>,
        runtime: &RuntimeService,
        recipe_registry: Arc<RecipeRegistry>,
    ) -> Self {
        let agent_runtime = AgentRuntimeService::new(
            runtime.agent_runtime_runtime(),
            runtime.agent_runtime_strategy_registry(),
            store.clone(),
        );
        Self {
            daemon_instance_id: runtime.daemon_instance_id(),
            runtime: runtime.clone(),
            agent_runtime: agent_runtime.clone(),
            run_execution: RunExecutionService::new(
                store.clone(),
                agent_runtime,
                runtime.run_execution_runtime(),
                Arc::clone(&recipe_registry),
            ),
            git_reverts: git::GitRevertRuntime::default(),
            code_host_pushes: code_host::CodeHostPushRuntime::default(),
            recipe_registry,
            work_source_refresh_requested: Arc::new(AtomicBool::new(false)),
            work_item_trigger_flights: Arc::new(Mutex::new(HashSet::new())),
            workflow: WorkflowManager::new(),
            store,
        }
    }

    pub(crate) fn rehydrate_run_scheduler_on_boot(&self) -> Result<(), AppServiceError> {
        self.run_execution
            .rehydrate_scheduler_on_boot()
            .map_err(map_run_execution_error)
    }

    pub(crate) fn reconcile_preparing_scheduled_work_on_boot(&self) -> Result<(), AppServiceError> {
        use ta_protocol::wire::ScheduledWorkOccurrenceState;
        let preparing = {
            let store = self.store.lock().expect("app store should not be poisoned");
            store
                .scheduled_work_occurrences()?
                .into_iter()
                .filter_map(|occurrence| match occurrence.state.clone() {
                    ScheduledWorkOccurrenceState::Preparing { run_id } => Some((
                        occurrence,
                        run_id,
                        ta_protocol::wire::ScheduledWorkPreparationTerminal::Failed,
                        "daemon restarted before scheduled preparation was published".to_string(),
                    )),
                    ScheduledWorkOccurrenceState::PreparationCancellationRequested {
                        run_id,
                        ..
                    } => Some((
                        occurrence,
                        run_id,
                        ta_protocol::wire::ScheduledWorkPreparationTerminal::Cancelled,
                        "scheduled preparation cancellation reconciled during daemon restart"
                            .to_string(),
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        for (occurrence, run_id, intended_terminal, detail) in preparing {
            let definition = {
                let store = self.store.lock().expect("app store should not be poisoned");
                store
                    .scheduled_work(&occurrence.scheduled_work_id)?
                    .ok_or_else(|| ta_store::StoreError::MissingRecord {
                        entity: "scheduled work",
                        key: occurrence.scheduled_work_id.as_str().to_string(),
                    })?
            };
            self.run_execution
                .reconcile_unpublished_scheduled_work(
                    &definition,
                    &occurrence.id,
                    &run_id,
                    intended_terminal,
                    detail,
                )
                .map_err(map_run_execution_error)?;
        }
        Ok(())
    }

    /// The single application lifecycle entry point used by bootstrap.  The
    /// store remains encapsulated here while each lower-level recovery action
    /// stays with its established owner.
    pub(crate) fn recover_on_boot(&self) -> Result<(), AppServiceError> {
        self.reconcile_preparing_scheduled_work_on_boot()?;
        crate::host::bootstrap::reconcile_orphaned_running_runs(&self.store)?;
        self.rehydrate_published_scheduled_resources_on_boot()?;
        self.rehydrate_run_scheduler_on_boot()
    }

    fn rehydrate_published_scheduled_resources_on_boot(&self) -> Result<(), AppServiceError> {
        use ta_protocol::wire::{RunSource, RunStatus};
        let runs = {
            let store = self.store.lock().expect("app store should not be poisoned");
            store
                .runs()?
                .into_iter()
                .filter(|run| {
                    matches!(run.source, RunSource::ScheduledWork { .. })
                        && !matches!(
                            run.status,
                            RunStatus::Completed
                                | RunStatus::Failed
                                | RunStatus::Cancelled
                                | RunStatus::BudgetExceeded
                        )
                })
                .collect::<Vec<_>>()
        };
        for run in runs {
            self.run_execution
                .rehydrate_published_scheduled_resources(&run)
                .map_err(map_run_execution_error)?;
        }
        Ok(())
    }

    pub(crate) fn register_navigation_sessions_for_principal(
        &self,
        principal_id: &str,
    ) -> Result<(), AppServiceError> {
        let principal_id = sanitize_session_owner_principal_id(principal_id)?;
        let sessions = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .sessions()?;
        for session in sessions
            .into_iter()
            .filter(|session| session.owner_principal_id == principal_id)
        {
            self.runtime
                .register_navigation_session(&session.id, &principal_id);
        }
        Ok(())
    }
}
