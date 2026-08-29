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
mod checkpoints;
mod completion_result;
mod continue_run;
#[cfg(test)]
mod e2e_hierarchical_replay_tests;
mod errors;
mod execution_context;
#[cfg(test)]
mod execution_context_tests;
mod fork_snapshot;
mod fresh_spawn;
#[cfg(test)]
mod media_tests;
mod native_children;
mod promote;
pub(crate) mod provider_sink;
#[cfg(test)]
mod recipes_e2e_tests;
mod resume;
mod run_fork;
#[cfg(test)]
mod run_fork_replay_tests;
mod scheduled_work;
mod start;
mod switch_account_and_resume;
#[cfg(test)]
pub(crate) mod test_support;

use ta_policy::PolicyDecision;
use ta_protocol::wire::{
    ApprovalEvent, ApprovalId, ApprovalRequest, ApprovalScope, ApprovalTarget, DaemonEvent,
    OutputContractKind, RecipeResolutionError, RunHarnessKind, RunId, RunRecord, RunSource,
    RunStatus, WorkspaceFileAttachment,
};
use ta_store::{EventRecord, InMemoryStore, PersistenceStore, RunProjection};
use taugentic_agent::AgentExecutionHarness;

pub use errors::RunExecutionError;
use errors::map_agent_runtime_error;
use execution_context::{ExecutionContextRequest, workspace_mode_for_fork};
use provider_sink::ProviderRunExecutionSink;

use crate::{
    AgentRuntimeService, ArtifactSummary, RecipeRegistry, RunExecutionRuntime, RunSummary,
};

pub struct RunExecutionService<S = InMemoryStore>
where
    S: PersistenceStore + Send + 'static,
{
    store: Arc<Mutex<S>>,
    agent_runtime: AgentRuntimeService<S>,
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
            agent_runtime: self.agent_runtime.clone(),
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

fn user_message_with_attachments(text: &str, attachments: &[WorkspaceFileAttachment]) -> String {
    if attachments.is_empty() {
        return text.to_string();
    }
    let manifest = attachments
        .iter()
        .filter(|attachment| attachment.kind != ta_protocol::wire::WorkspaceFileKind::Image)
        .map(|attachment| {
            serde_json::json!({
                "path": attachment.path,
                "revision": attachment.revision,
                "kind": attachment.kind,
                "byteLength": attachment.byte_len.to_string(),
            })
        })
        .collect::<Vec<_>>();
    if manifest.is_empty() {
        return text.to_string();
    }
    format!(
        "{text}\n\n<taugentic_workspace_attachments>\n{}\n</taugentic_workspace_attachments>",
        serde_json::to_string(&manifest).expect("attachment manifest should serialize")
    )
}

impl<S> RunExecutionService<S>
where
    S: PersistenceStore + Send + 'static,
{
    pub(crate) fn new(
        store: Arc<Mutex<S>>,
        agent_runtime: AgentRuntimeService<S>,
        runtime: RunExecutionRuntime,
        recipe_registry: Arc<RecipeRegistry>,
    ) -> Self {
        Self {
            store,
            agent_runtime,
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

    pub(crate) fn artifact_root(&self) -> &std::path::Path {
        self.runtime.artifact_root()
    }

    pub(crate) fn discard_unpublished_scheduled_resources(
        &self,
        run_id: &RunId,
        repo_root: &std::path::Path,
    ) -> Result<(), RunExecutionError> {
        self.runtime
            .discard_unpublished_scheduled_resources(run_id, repo_root)
            .map_err(map_agent_runtime_error)
    }

    pub(crate) fn unpublished_scheduled_resource(
        &self,
        run_id: &RunId,
        repo_root: &std::path::Path,
        cleanup_policy: ta_protocol::wire::WorktreeCleanupPolicy,
    ) -> Result<ta_protocol::wire::ScheduledWorkUnpublishedResource, RunExecutionError> {
        self.runtime
            .unpublished_scheduled_resource(run_id, repo_root, cleanup_policy)
            .map_err(map_agent_runtime_error)
    }

    pub(crate) fn rehydrate_published_scheduled_resources(
        &self,
        run: &RunProjection,
    ) -> Result<(), RunExecutionError> {
        self.runtime
            .rehydrate_published_scheduled_resources(run)
            .map_err(map_agent_runtime_error)
    }

    pub(crate) fn is_voice_run(&self, run_id: &RunId) -> bool {
        self.runtime.is_voice_run(run_id)
    }

    pub(crate) fn exchange_voice_frame(
        &self,
        run_id: &RunId,
        input: [u8; ta_protocol::wire::VOICE_FRAME_BYTES],
        playback_completed_frames: u64,
    ) -> Result<crate::orchestration::voice::VoiceExchange, RunExecutionError> {
        self.runtime
            .exchange_voice_frame(run_id, input, playback_completed_frames)
            .map_err(RunExecutionError::ProviderExecutionFailed)
    }

    pub(crate) fn end_voice(
        &self,
        run_id: &RunId,
        reason: ta_protocol::wire::VoiceStreamEndReason,
    ) -> Result<(), RunExecutionError> {
        self.runtime
            .end_voice(run_id, reason)
            .map_err(RunExecutionError::ProviderExecutionFailed)
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
            vec![DaemonEvent::Run(
                crate::RunEvent::terminal(
                    run_id,
                    RunStatus::Failed,
                    crate::RunStatusReason::new(reason).expect("policy reason"),
                    None,
                    recipe_id,
                    None,
                )
                .expect("failed is terminal"),
            )],
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
                    DaemonEvent::Run(
                        crate::RunEvent::active(
                            run_id,
                            RunStatus::WaitingForApproval,
                            None,
                            recipe_id,
                            None,
                        )
                        .expect("active status"),
                    ),
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
        vec![DaemonEvent::Run(
            crate::RunEvent::active(run_id, RunStatus::Running, None, recipe_id, None)
                .expect("active status"),
        )],
    )
}

fn build_queue_transition(
    run_id: RunId,
    position: usize,
    recipe_id: Option<String>,
) -> (RunStatus, Vec<DaemonEvent>) {
    (
        RunStatus::Queued,
        vec![DaemonEvent::Run(
            crate::RunEvent::active(run_id, RunStatus::Queued, None, recipe_id, None)
                .expect("active status"),
        )],
    )
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
        | RunSource::FreshSpawn { parent_run_id, .. }
        | RunSource::Forked { parent_run_id, .. }
        | RunSource::AccountSwitchedContinuation { parent_run_id, .. } => {
            Some(parent_run_id.clone())
        }
        RunSource::ScheduledWork { .. } | RunSource::User { .. } => None,
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
        | RunSource::FreshSpawn {
            output_contract, ..
        }
        | RunSource::User {
            output_contract, ..
        } => *output_contract,
        RunSource::ScheduledWork { .. }
        | RunSource::Forked { .. }
        | RunSource::AccountSwitchedContinuation { .. } => None,
    }
}

fn recipe_id_for_run(run: &RunProjection) -> Option<String> {
    match &run.source {
        RunSource::NativeSubagent { recipe_id, .. }
        | RunSource::FreshSpawn { recipe_id, .. }
        | RunSource::User { recipe_id, .. } => recipe_id.clone(),
        RunSource::ScheduledWork { .. }
        | RunSource::Forked { .. }
        | RunSource::AccountSwitchedContinuation { .. } => None,
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

pub(crate) fn run_harness_kind(harness: &AgentExecutionHarness) -> RunHarnessKind {
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

#[cfg(test)]
mod scheduled_work_contract_tests {
    use super::*;
    use ta_protocol::wire::{ScheduledWorkId, ScheduledWorkOccurrenceId, SessionId};

    fn scheduled_run() -> RunProjection {
        let context = ta_store::default_test_execution_context();
        RunProjection {
            id: RunId::new("run-scheduled-contract").expect("run id"),
            session_id: SessionId::new("session-scheduled-contract").expect("session id"),
            runtime_profile_id: ta_store::default_test_run_source()
                .route()
                .runtime_profile_id
                .clone(),
            objective: "Scheduled root".to_string(),
            status: RunStatus::Queued,
            harness: RunHarnessKind::Native,
            source: RunSource::ScheduledWork {
                route: ta_store::default_test_run_source().route().clone(),
                scheduled_work_id: ScheduledWorkId::new("schedule-contract").expect("schedule id"),
                occurrence_id: ScheduledWorkOccurrenceId::new("occurrence-contract")
                    .expect("occurrence id"),
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
        }
    }

    #[test]
    fn scheduled_work_is_root_without_inherited_contract() {
        let run = scheduled_run();
        let record = project_run_record(run.clone());
        assert_eq!(record.parent_run_id, None);
        assert_eq!(output_contract_for_run(&run), None);
        assert_eq!(recipe_id_for_run(&run), None);
    }
}
