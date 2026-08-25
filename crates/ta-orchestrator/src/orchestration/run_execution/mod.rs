use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

mod approval;
mod artifacts;
mod budget;
#[cfg(test)]
mod budget_tests;
mod cancel;
mod completion_result;
#[cfg(test)]
mod e2e_hierarchical_replay_tests;
mod errors;
mod execution_context;
#[cfg(test)]
mod execution_context_tests;
mod fork_snapshot;
mod native_children;
mod promote;
mod provider_sink;
#[cfg(test)]
mod recipes_e2e_tests;
mod resume;
mod run_fork;
#[cfg(test)]
mod run_fork_replay_tests;
mod start;
#[cfg(test)]
mod test_support;

use ta_policy::PolicyDecision;
use ta_protocol::wire::{
    AgentRuntimeModelId, ApprovalEvent, ApprovalId, ApprovalRequest, ApprovalScope, ApprovalTarget,
    DaemonEvent, OutputContractKind, RecipeResolutionError, RunHarnessKind, RunId, RunRecord,
    RunSource, RunStatus,
};
use ta_store::{ArtifactRecord, EventRecord, InMemoryStore, PersistenceStore, RunProjection};
use taugentic_agent::AgentExecutionHarness;

pub use errors::RunExecutionError;
use errors::map_agent_runtime_error;
use execution_context::{ExecutionContextRequest, workspace_mode_for_fork};
use provider_sink::ProviderRunExecutionSink;

use crate::{ArtifactSummary, RecipeRegistry, RunExecutionRuntime, RunSummary};

pub struct RunExecutionService<S = InMemoryStore>
where
    S: PersistenceStore + Send + 'static,
{
    store: Arc<Mutex<S>>,
    runtime: RunExecutionRuntime,
    recipe_registry: Arc<RecipeRegistry>,
}

impl<S> Clone for RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            runtime: self.runtime.clone(),
            recipe_registry: Arc::clone(&self.recipe_registry),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunMutationResult {
    pub run: RunSummary,
    pub events: Vec<EventRecord>,
}

impl RunMutationResult {
    #[cfg(test)]
    fn requested_approval_id(&self) -> Option<ta_protocol::wire::ApprovalId> {
        self.events.iter().find_map(|record| match &record.payload {
            DaemonEvent::Approval(ApprovalEvent::Requested { request }) => Some(request.id.clone()),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactMutationResult {
    pub artifact: ArtifactSummary,
    pub events: Vec<EventRecord>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ExecutionRequestOverrides {
    model_id: Option<AgentRuntimeModelId>,
}

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(crate) fn new(
        store: Arc<Mutex<S>>,
        runtime: RunExecutionRuntime,
        recipe_registry: Arc<RecipeRegistry>,
    ) -> Self {
        Self {
            store,
            runtime,
            recipe_registry,
        }
    }

    pub(crate) fn active_run_count(&self) -> usize {
        self.runtime.active_run_count()
    }

    pub(crate) fn workspace_run_count(&self) -> usize {
        self.runtime.workspace_run_count()
    }

    pub(crate) fn claim_count(&self) -> usize {
        self.runtime.claim_count()
    }

    pub(super) fn load_run_projection(
        &self,
        run_id: &RunId,
    ) -> Result<RunProjection, RunExecutionError> {
        let store = self.store.lock().expect("app store should not be poisoned");
        store
            .run(run_id)?
            .ok_or_else(|| RunExecutionError::RunNotFound(run_id.as_str().to_string()))
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn is_live_run_running(
        &self,
        run_id: &RunId,
        session_id: &crate::SessionId,
    ) -> bool {
        self.runtime.is_live_run_running(run_id, session_id)
    }
}

fn build_start_transition(
    run_id: RunId,
    decision: PolicyDecision,
    recipe_id: Option<String>,
) -> (RunStatus, Vec<DaemonEvent>) {
    match decision {
        PolicyDecision::Allow => start_running_transition(run_id, recipe_id),
        PolicyDecision::Deny { reason } => (
            RunStatus::Failed,
            vec![DaemonEvent::Run(crate::RunEvent {
                run_id,
                status: RunStatus::Failed,
                detail: reason,
                output_contract: None,
                recipe_id,
                result: None,
            })],
        ),
        PolicyDecision::RequireApproval { reason } => {
            let requested_at_ms = current_time_ms();
            let ttl = ta_policy::ApprovalTtlPolicy::default();
            let approval = ApprovalRequest::new(
                ApprovalId::new(format!("approval-{}", uuid::Uuid::new_v4().simple()))
                    .expect("generated approval id should be valid"),
                run_id.clone(),
                ApprovalScope::ProcessExec,
                requested_at_ms,
                ttl.expires_at_ms(requested_at_ms),
                ApprovalTarget::ProcessExec { command: None },
                reason,
            )
            .expect("generated approval request should be valid");
            (
                RunStatus::WaitingForApproval,
                vec![
                    DaemonEvent::Run(crate::RunEvent {
                        run_id,
                        status: RunStatus::WaitingForApproval,
                        detail: "Waiting for approval".to_string(),
                        output_contract: None,
                        recipe_id,
                        result: None,
                    }),
                    DaemonEvent::Approval(ApprovalEvent::Requested { request: approval }),
                ],
            )
        }
    }
}

fn start_running_transition(
    run_id: RunId,
    recipe_id: Option<String>,
) -> (RunStatus, Vec<DaemonEvent>) {
    (
        RunStatus::Running,
        vec![DaemonEvent::Run(crate::RunEvent {
            run_id,
            status: RunStatus::Running,
            detail: "Execution started".to_string(),
            output_contract: None,
            recipe_id,
            result: None,
        })],
    )
}

fn build_queue_transition(
    run_id: RunId,
    position: usize,
    recipe_id: Option<String>,
) -> (RunStatus, Vec<DaemonEvent>) {
    (
        RunStatus::Queued,
        vec![DaemonEvent::Run(crate::RunEvent {
            run_id,
            status: RunStatus::Queued,
            detail: format!("Queued behind active run at position {position}"),
            output_contract: None,
            recipe_id,
            result: None,
        })],
    )
}

fn project_artifact_summary(artifact: ArtifactRecord) -> ArtifactSummary {
    ArtifactSummary {
        id: artifact.id,
        run_id: artifact.run_id,
        kind: artifact.kind,
        storage_path: artifact.storage_path,
    }
}

fn project_run_summary(run: RunProjection) -> RunSummary {
    RunSummary {
        id: run.id,
        runtime_profile_id: run.runtime_profile_id,
        objective: run.objective,
        status: run.status,
    }
}

fn project_run_record(run: RunProjection) -> RunRecord {
    let parent_run_id = match &run.source {
        RunSource::NativeSubagent { parent_run_id, .. }
        | RunSource::Forked { parent_run_id, .. } => Some(parent_run_id.clone()),
        RunSource::User { .. } => None,
    };
    RunRecord {
        id: run.id,
        session_id: run.session_id,
        parent_run_id,
        runtime_profile_id: run.runtime_profile_id,
        objective: run.objective,
        status: run.status,
        harness: run.harness,
        source: run.source,
        execution_context: run.execution_context,
        started_at_ms: run.started_at_ms,
        ended_at_ms: run.ended_at_ms,
        last_event_seq: run.last_event_seq,
        workspace_info: run.workspace_info,
        claimed_files: run.claimed_files,
        conflict_summary: run.conflict_summary,
    }
}

fn output_contract_for_run(run: &RunProjection) -> Option<OutputContractKind> {
    match &run.source {
        RunSource::NativeSubagent {
            output_contract, ..
        }
        | RunSource::User {
            output_contract, ..
        } => *output_contract,
        RunSource::Forked { .. } => None,
    }
}

fn execution_overrides_for_run(run: &RunProjection) -> ExecutionRequestOverrides {
    match &run.source {
        RunSource::NativeSubagent { model_id, .. } | RunSource::User { model_id, .. } => {
            ExecutionRequestOverrides {
                model_id: model_id.clone(),
            }
        }
        RunSource::Forked { .. } => ExecutionRequestOverrides::default(),
    }
}

fn recipe_id_for_run(run: &RunProjection) -> Option<String> {
    match &run.source {
        RunSource::NativeSubagent { recipe_id, .. } | RunSource::User { recipe_id, .. } => {
            recipe_id.clone()
        }
        RunSource::Forked { .. } => None,
    }
}

fn map_recipe_resolution_error(error: RecipeResolutionError) -> RunExecutionError {
    match error {
        RecipeResolutionError::UnknownRecipeId { recipe_id } => {
            RunExecutionError::UnknownRecipeId(recipe_id)
        }
        RecipeResolutionError::RecipeContractConflict {
            recipe_id,
            recipe_contract,
            request_contract,
        } => RunExecutionError::RecipeContractConflict {
            recipe_id,
            recipe_contract,
            request_contract,
        },
    }
}

fn run_harness_kind(harness: &AgentExecutionHarness) -> RunHarnessKind {
    match harness {
        AgentExecutionHarness::NativeLoop => RunHarnessKind::Native,
        AgentExecutionHarness::Acp { .. } => RunHarnessKind::Acp,
        AgentExecutionHarness::CodexAppServer => RunHarnessKind::CodexAppServer,
    }
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis() as u64
}
