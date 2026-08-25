use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ta_policy::{Operation, PolicyDecision, PolicyEngine};
use ta_store::EventRecord;
use taugentic_agent::{
    AgentExecutionHarness, ExecutionError, ExecutionHandle, ExecutionRequest, ExecutionSink,
    ForkInitialState,
};

#[cfg(test)]
use super::active_execution::ActiveExecution;
use super::active_execution::{ActiveExecutionOwner, AttachHandleDisposition};
use crate::host::event_hub::RuntimeEventPublisher;
use crate::orchestration::agent_runtime::{StrategyRegistry, validate_runtime_profile};
use crate::orchestration::prompts::subagent_delegation_guidelines;
use crate::workspace::{
    ClaimHandle, ClaimKind, ClaimRegistry, CleanupPolicy, WorktreeHandle, WorktreeManager,
    WorktreeRequest,
};
use crate::{
    AgentRuntimeRuntime, AgentRuntimeServiceError, DaemonEventEnvelope, LaneCapabilities, RunId,
    RunScheduleDisposition, RunScheduler, RunSchedulerError, RunSchedulingPolicy,
    SchedulerRehydratePlan, SessionId,
};

#[derive(Clone)]
pub(crate) struct RunExecutionRuntime {
    pub(super) capabilities: LaneCapabilities,
    policy: AgentRuntimeRuntime,
    strategy_registry: StrategyRegistry,
    active_executions: ActiveExecutionOwner,
    scheduler: RunScheduler,
    event_publisher: RuntimeEventPublisher,
    artifact_root: PathBuf,
    worktree_manager: WorktreeManager,
    claim_registry: ClaimRegistry,
    workspace_runs: WorkspaceRunRegistry,
    budget_policy: Arc<Mutex<ta_policy::BudgetPolicy>>,
}

#[derive(Debug, Clone)]
pub(crate) struct DispatchWorkspace {
    pub effective_cwd: PathBuf,
    pub worktree_info: Option<ta_protocol::wire::WorktreeInfo>,
    pub claimed_files: Vec<String>,
    pub conflict_warning: Option<ta_protocol::wire::ConflictWarning>,
}

#[derive(Clone)]
struct WorkspaceRunRegistry {
    inner: Arc<Mutex<BTreeMap<RunId, WorkspaceRunResources>>>,
}

struct WorkspaceRunResources {
    worktree: Option<WorktreeHandle>,
    claim: Option<ClaimHandle>,
}

#[derive(Debug, Clone)]
pub struct RuntimeExecutionPaths {
    pub artifact_root: PathBuf,
}

pub(crate) struct ProviderRunStart<'a> {
    pub runtime_profile: &'a ta_protocol::wire::RuntimeProfileSummary,
    pub session_id: &'a SessionId,
    pub run_id: &'a RunId,
    pub objective: &'a str,
    pub execution_context: Arc<ta_protocol::wire::ExecutionContext>,
    pub fork_initial_state: Option<ForkInitialState>,
    pub output_contract: Option<ta_protocol::wire::OutputContractKind>,
    pub model_id: Option<&'a ta_protocol::wire::AgentRuntimeModelId>,
    pub subagent_recipes: Vec<ta_protocol::wire::CapsuleRecipe>,
}

pub async fn execute_run(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
) -> Result<Arc<dyn ExecutionHandle>, ExecutionError> {
    taugentic_agent::run(request, sink).await
}

impl RuntimeExecutionPaths {
    pub(super) fn from_current_process() -> Self {
        let working_directory =
            std::env::current_dir().expect("current working directory should be available");
        let artifact_root = working_directory.join("target/daemon-artifacts");
        Self { artifact_root }
    }
}

impl std::fmt::Debug for RunExecutionRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RunExecutionRuntime")
            .field("capabilities", &self.capabilities)
            .field("artifact_root", &self.artifact_root)
            .finish_non_exhaustive()
    }
}

impl WorkspaceRunRegistry {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    fn insert(&self, run_id: RunId, resources: WorkspaceRunResources) {
        self.inner
            .lock()
            .expect("workspace run registry should not be poisoned")
            .insert(run_id, resources);
    }

    fn finish(&self, run_id: &RunId, status: ta_protocol::wire::RunStatus) {
        let resources = self
            .inner
            .lock()
            .expect("workspace run registry should not be poisoned")
            .remove(run_id);
        let Some(resources) = resources else {
            return;
        };
        drop(resources.claim);
        if let Some(worktree) = resources.worktree {
            match status {
                ta_protocol::wire::RunStatus::Completed => worktree.mark_success(),
                ta_protocol::wire::RunStatus::Failed
                | ta_protocol::wire::RunStatus::BudgetExceeded => worktree.mark_failed(),
                ta_protocol::wire::RunStatus::Cancelled => worktree.mark_cancelled(),
                _ => worktree.mark_terminal(),
            }
            drop(worktree);
        }
    }

    fn active_count(&self) -> usize {
        self.inner
            .lock()
            .expect("workspace run registry should not be poisoned")
            .len()
    }
}

impl RunExecutionRuntime {
    pub(super) fn new(
        capabilities: LaneCapabilities,
        policy: AgentRuntimeRuntime,
        strategy_registry: StrategyRegistry,
        event_publisher: RuntimeEventPublisher,
        execution_paths: RuntimeExecutionPaths,
    ) -> Self {
        Self {
            capabilities,
            policy,
            strategy_registry,
            active_executions: ActiveExecutionOwner::new(),
            scheduler: RunScheduler::new(),
            event_publisher,
            artifact_root: execution_paths.artifact_root,
            worktree_manager: WorktreeManager::with_git_binary(PathBuf::from("git")),
            claim_registry: ClaimRegistry::new(),
            workspace_runs: WorkspaceRunRegistry::new(),
            budget_policy: Arc::new(Mutex::new(ta_policy::BudgetPolicy::default())),
        }
    }

    pub(crate) fn evaluate_operation(
        &self,
        operation: &Operation,
    ) -> Result<PolicyDecision, AgentRuntimeServiceError> {
        Ok(
            PolicyEngine::from_runtime_policy_mode(self.policy.policy_mode()?)
                .evaluate(operation, self.capabilities.supports_network),
        )
    }

    pub(crate) fn evaluate_operation_for_policy_mode(
        &self,
        operation: &Operation,
        policy_mode: ta_protocol::wire::RuntimePolicyMode,
    ) -> PolicyDecision {
        PolicyEngine::from_runtime_policy_mode(policy_mode)
            .evaluate(operation, self.capabilities.supports_network)
    }

    pub(crate) fn selected_runtime_profile(
        &self,
    ) -> Result<ta_protocol::wire::RuntimeProfileSummary, AgentRuntimeServiceError> {
        let profile = self.policy.selected_profile()?;
        validate_runtime_profile(&profile, &self.strategy_registry)
    }

    pub(crate) fn runtime_profile(
        &self,
        runtime_profile_id: &crate::RuntimeProfileId,
    ) -> Result<ta_protocol::wire::RuntimeProfileSummary, AgentRuntimeServiceError> {
        let profile = self
            .policy
            .runtime_profile(runtime_profile_id)
            .ok_or_else(|| {
                AgentRuntimeServiceError::RuntimeProfileNotFound(
                    runtime_profile_id.as_str().to_string(),
                )
            })?;
        validate_runtime_profile(&profile, &self.strategy_registry)
    }

    pub(crate) fn execution_harness_for_runtime_profile(
        &self,
        runtime_profile: &ta_protocol::wire::RuntimeProfileSummary,
    ) -> Result<AgentExecutionHarness, AgentRuntimeServiceError> {
        self.strategy_registry
            .execution_harness_for_runtime_profile(runtime_profile)
    }

    pub(super) fn build_execution_request(
        &self,
        start: ProviderRunStart<'_>,
    ) -> Result<ExecutionRequest, crate::orchestration::AgentRuntimeServiceError> {
        let ProviderRunStart {
            runtime_profile,
            session_id,
            run_id,
            objective,
            execution_context,
            fork_initial_state,
            output_contract,
            model_id,
            subagent_recipes,
        } = start;
        let runtime_profile = validate_runtime_profile(runtime_profile, &self.strategy_registry)?;
        let execution_harness = self
            .strategy_registry
            .execution_harness_for_runtime_profile(&runtime_profile)?;
        let system_prompt =
            system_prompt_for_execution_request(&execution_harness, subagent_recipes.len());

        Ok(ExecutionRequest {
            session_id: session_id.clone(),
            run_id: run_id.clone(),
            runtime_profile_id: runtime_profile.id.clone(),
            provider_id: runtime_profile.provider_id.clone(),
            execution_harness,
            system_prompt,
            objective: objective.to_string(),
            model_id: model_id
                .cloned()
                .or_else(|| runtime_profile.model_id.clone()),
            auth_profile_id: runtime_profile.auth_profile_id.clone(),
            resume_provider_session_id: None,
            runtime_extensions: self.policy.runtime_extensions(),
            execution_context,
            fork_initial_state,
            output_contract,
            subagent_recipes,
        })
    }

    pub(crate) fn start_provider_run(
        &self,
        start: ProviderRunStart<'_>,
        sink: Arc<dyn ExecutionSink>,
    ) -> Result<(), crate::orchestration::AgentRuntimeServiceError> {
        let run_id = start.run_id;
        let request = self.build_execution_request(start)?;
        let handle = execute_run_sync(request, sink)?;
        self.active_executions
            .attach_handle(run_id, handle)
            .map_err(|error| {
                crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed(error)
            })
            .map(|disposition| match disposition {
                AttachHandleDisposition::Attached => (),
                AttachHandleDisposition::CancelRequested => (),
            })
    }

    pub(crate) fn claim_live_run(&self, run_id: RunId, session_id: SessionId) {
        self.active_executions.claim_run(run_id, session_id);
    }

    pub(crate) fn active_run_count(&self) -> usize {
        self.active_executions.active_count()
    }

    pub(crate) fn workspace_run_count(&self) -> usize {
        self.workspace_runs.active_count()
    }

    pub(crate) fn claim_count(&self) -> usize {
        self.claim_registry.active_claims().len()
    }

    pub(crate) fn schedule_run_start(
        &self,
        session_id: &SessionId,
        run_id: RunId,
    ) -> Result<RunScheduleDisposition, RunSchedulerError> {
        self.scheduler.schedule_start(session_id, run_id)
    }

    pub(crate) fn schedule_run_start_with_policy(
        &self,
        session_id: &SessionId,
        run_id: RunId,
        policy: RunSchedulingPolicy,
    ) -> Result<RunScheduleDisposition, RunSchedulerError> {
        self.scheduler
            .schedule_start_with_policy(session_id, run_id, policy)
    }

    pub(crate) fn allocate_execution_workspace(
        &self,
        run_id: &RunId,
        parent_repo: &std::path::Path,
        workspace_root: &std::path::Path,
        workspace_scope: ta_protocol::wire::WorkspaceMode,
        cleanup_policy: ta_protocol::wire::WorktreeCleanupPolicy,
        planned_write_files: &[String],
    ) -> Result<DispatchWorkspace, crate::orchestration::AgentRuntimeServiceError> {
        let mut worktree = None;
        let mut worktree_info = None;
        let effective_cwd = match workspace_scope {
            ta_protocol::wire::WorkspaceMode::WorktreeWrite => {
                let handle = self
                    .worktree_manager
                    .create(WorktreeRequest {
                        parent_repo: parent_repo.to_path_buf(),
                        capsule_short_id: run_id.as_str().to_string(),
                        recipe_hint: None,
                        cleanup_policy: cleanup_policy.into(),
                    })
                    .map_err(|error| {
                        crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed(
                            error.to_string(),
                        )
                    })?;
                let worktree_root = handle.path().to_path_buf();
                let workspace_relative_path =
                    workspace_root.strip_prefix(parent_repo).map_err(|_| {
                        crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed(
                            format!(
                                "workspace root {} is outside git repository {}",
                                workspace_root.display(),
                                parent_repo.display()
                            ),
                        )
                    })?;
                let path = worktree_root.join(workspace_relative_path);
                if !path.is_dir() {
                    return Err(
                        crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed(
                            format!(
                                "workspace path {} does not exist in prepared worktree",
                                path.display()
                            ),
                        ),
                    );
                }
                worktree_info = Some(ta_protocol::wire::WorktreeInfo {
                    path: worktree_root.to_string_lossy().into_owned(),
                    branch: handle.branch().to_string(),
                    cleanup_policy,
                });
                worktree = Some(handle);
                path
            }
            _ => workspace_root.to_path_buf(),
        };

        let (claim, claimed_files, conflict_warning) = if planned_write_files.is_empty() {
            (None, Vec::new(), None)
        } else {
            let files = planned_write_files.iter().map(PathBuf::from).collect();
            let (claim, warning) = self
                .claim_registry
                .claim(run_id.clone(), files, None, ClaimKind::Write)
                .map_err(|error| {
                    crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed(
                        error.to_string(),
                    )
                })?;
            let claimed_files = claim
                .files()
                .iter()
                .map(|path| protocol_workspace_path(path))
                .collect();
            (
                Some(claim),
                claimed_files,
                warning.map(protocol_conflict_warning),
            )
        };

        if worktree.is_some() || claim.is_some() {
            self.workspace_runs
                .insert(run_id.clone(), WorkspaceRunResources { worktree, claim });
        }

        Ok(DispatchWorkspace {
            effective_cwd,
            worktree_info,
            claimed_files,
            conflict_warning,
        })
    }

    pub(crate) fn artifact_root(&self) -> &std::path::Path {
        &self.artifact_root
    }

    pub(crate) fn supports_network(&self) -> bool {
        self.capabilities.supports_network
    }

    pub(crate) fn is_live_run_running(&self, run_id: &RunId, session_id: &SessionId) -> bool {
        self.active_executions
            .is_running_owned_by(run_id, session_id)
    }

    pub(crate) fn cancel_live_run(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
    ) -> Result<(), crate::orchestration::AgentRuntimeServiceError> {
        self.active_executions
            .cancel_run(run_id, session_id)
            .map_err(crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed)
    }

    pub(crate) fn resolve_live_approval(
        &self,
        run_id: &RunId,
        session_id: &SessionId,
        resolution: ta_protocol::wire::ApprovalResolution,
    ) -> Result<(), crate::orchestration::AgentRuntimeServiceError> {
        self.active_executions
            .resolve_approval(run_id, session_id, resolution)
            .map_err(crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn release_live_run(&self, run_id: &RunId) -> bool {
        self.active_executions.release_run(run_id)
    }

    #[cfg(test)]
    pub(crate) fn attach_live_run_handle_for_tests(
        &self,
        run_id: &RunId,
        handle: Arc<dyn ExecutionHandle>,
    ) -> Result<(), String> {
        self.active_executions
            .attach_handle(run_id, handle)
            .map(|_| ())
    }

    pub(crate) fn finish_scheduled_run(
        &self,
        session_id: &SessionId,
        run_id: &RunId,
        status: ta_protocol::wire::RunStatus,
    ) -> Option<RunId> {
        self.active_executions.release_run(run_id);
        self.workspace_runs.finish(run_id, status);
        self.scheduler.finish_run(session_id, run_id)
    }

    pub(crate) fn rehydrate_scheduler_from_store<S>(
        &self,
        store: &S,
    ) -> Result<SchedulerRehydratePlan, ta_store::StoreError>
    where
        S: ta_store::PersistenceStore + Send,
    {
        self.scheduler.rehydrate_from_store(store)
    }

    #[cfg(test)]
    pub(super) fn live_execution_for(&self, run_id: &RunId) -> Option<ActiveExecution> {
        self.active_executions.execution_for(run_id)
    }

    pub(crate) fn publish_record(&self, record: &EventRecord) -> DaemonEventEnvelope {
        self.event_publisher.publish(record)
    }

    pub(crate) fn budget_policy(&self) -> ta_policy::BudgetPolicy {
        *self
            .budget_policy
            .lock()
            .expect("budget policy should not be poisoned")
    }

    #[cfg(test)]
    pub(crate) fn set_budget_policy_for_tests(&self, policy: ta_policy::BudgetPolicy) {
        *self
            .budget_policy
            .lock()
            .expect("budget policy should not be poisoned") = policy;
    }
}

fn system_prompt_for_execution_request(
    execution_harness: &AgentExecutionHarness,
    recipe_count: usize,
) -> Option<String> {
    if execution_harness.is_native() {
        return Some(subagent_delegation_guidelines(recipe_count));
    }
    None
}

fn execute_run_sync(
    request: ExecutionRequest,
    sink: Arc<dyn ExecutionSink>,
) -> Result<Arc<dyn ExecutionHandle>, crate::orchestration::AgentRuntimeServiceError> {
    std::thread::Builder::new()
        .name(format!("taugentic-agent-run-{}", request.run_id.as_str()))
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| ExecutionError::ProcessFailed(error.to_string()))?;
            runtime.block_on(execute_run(request, sink))
        })
        .map_err(|error| {
            crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed(format!(
                "failed to spawn taugentic-agent run thread: {error}",
            ))
        })?
        .join()
        .map_err(|_| {
            crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed(
                "taugentic-agent run thread panicked".to_string(),
            )
        })?
        .map_err(|error| {
            crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed(
                error.to_string(),
            )
        })
}

impl From<ta_protocol::wire::WorktreeCleanupPolicy> for CleanupPolicy {
    fn from(value: ta_protocol::wire::WorktreeCleanupPolicy) -> Self {
        match value {
            ta_protocol::wire::WorktreeCleanupPolicy::DeleteOnSuccess => Self::DeleteOnSuccess,
            ta_protocol::wire::WorktreeCleanupPolicy::DeleteOnTerminal => Self::DeleteOnTerminal,
            ta_protocol::wire::WorktreeCleanupPolicy::Keep => Self::Keep,
            ta_protocol::wire::WorktreeCleanupPolicy::Manual => Self::Manual,
        }
    }
}

fn protocol_conflict_warning(
    warning: crate::workspace::ConflictWarning,
) -> ta_protocol::wire::ConflictWarning {
    ta_protocol::wire::ConflictWarning {
        requesting_capsule: warning.requesting_capsule,
        severity: ta_protocol::wire::ConflictSeverity::Warning,
        conflicts: warning
            .conflicts
            .into_iter()
            .map(|conflict| ta_protocol::wire::FileClaimConflict {
                file: protocol_workspace_path(&conflict.file),
                holding_capsule: conflict.holding_capsule,
                holding_kind: match conflict.holding_kind {
                    crate::workspace::ClaimKind::Write => ta_protocol::wire::FileClaimKind::Write,
                },
            })
            .collect(),
    }
}

fn protocol_workspace_path(path: &std::path::Path) -> String {
    platform_protocol_workspace_path(path)
}

#[cfg(windows)]
fn platform_protocol_workspace_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(not(windows))]
fn platform_protocol_workspace_path(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}
