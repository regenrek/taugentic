use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::wire::{
    AgentRuntimeSelection, EnvPolicy, ExecutionContext, NetworkPolicy, PermissionPolicy,
    RunExecutionRoute, RunId, SandboxProfile, ScheduledWorkId, ScheduledWorkOccurrenceId,
    SessionId, WorkspaceId, WorkspaceMode, WorkspacePath, WorkspaceScope, WorktreeCleanupPolicy,
};

/// The only durable scheduled-work product shape in the first vertical. It is
/// deliberately a one-shot, existing-conversation request: no cron, editing,
/// provider inference, or credential bytes can enter this record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ScheduledWorkDefinition {
    pub id: ScheduledWorkId,
    pub session_id: SessionId,
    pub objective: String,
    pub route: RunExecutionRoute,
    pub execution_request: ScheduledWorkExecutionRequest,
    #[serde(with = "crate::wire::u64_string")]
    #[schemars(schema_with = "crate::wire::u64_string::json_schema")]
    #[ts(type = "string")]
    pub due_at_ms: u64,
    pub attention_policy: ScheduledWorkAttentionPolicy,
}

impl ScheduledWorkDefinition {
    pub fn validate(&self) -> Result<(), ScheduledWorkValidationError> {
        if self.objective.trim().is_empty() {
            return Err(ScheduledWorkValidationError::EmptyObjective);
        }
        Ok(())
    }
}

/// The durable, non-secret execution policy frozen for scheduled work. It is
/// deliberately not an [`ExecutionContext`].  It is the complete non-secret
/// input to the sole run-specific preparer; it contains no credential bytes
/// and does not permit dispatch-time policy compilation or defaulting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ScheduledWorkExecutionRequest {
    pub workspace_id: WorkspaceId,
    pub workspace_root: WorkspacePath,
    pub repo_root: WorkspacePath,
    pub artifact_root: WorkspacePath,
    pub workspace_mode: WorkspaceMode,
    pub cleanup_policy: WorktreeCleanupPolicy,
    pub planned_write_files: Vec<String>,
    pub workspace_scope: WorkspaceScope,
    pub sandbox_profile: SandboxProfile,
    pub permission_policy: PermissionPolicy,
    pub network_policy: NetworkPolicy,
    pub env_policy: EnvPolicy,
}

impl ScheduledWorkExecutionRequest {
    pub fn matches_execution_context(&self, context: &ExecutionContext) -> bool {
        self.workspace_id == context.workspace_id
            && self.workspace_root == context.workspace_root
            && self.workspace_scope == context.workspace_scope
            && self.sandbox_profile == context.sandbox_profile
            && self.permission_policy == context.permission_policy
            && self.network_policy == context.network_policy
            && self.env_policy == context.env_policy
    }
}

/// Exact deterministic identity of the only unpublished external resource a
/// preparation may own.  It is durable solely when cleanup needs human
/// intervention; it intentionally contains no credentials or provider data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ScheduledWorkUnpublishedResource {
    pub parent_repo: String,
    pub worktree_path: String,
    pub branch: String,
    pub cleanup_policy: WorktreeCleanupPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ScheduledWorkAttentionPolicy {
    AttentionOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ScheduledWorkOccurrenceState {
    Pending,
    Preparing {
        run_id: RunId,
    },
    PreparationCancellationRequested {
        run_id: RunId,
        resource: ScheduledWorkUnpublishedResource,
    },
    Claimed {
        run_id: RunId,
    },
    Completed {
        run_id: RunId,
    },
    Failed {
        run_id: RunId,
    },
    BudgetExceeded {
        run_id: RunId,
    },
    Cancelled {
        run_id: Option<RunId>,
    },
    PreparationFailed {
        run_id: RunId,
        detail: String,
    },
    PreparationCancelled {
        run_id: RunId,
    },
    CleanupRequired {
        run_id: RunId,
        resource: ScheduledWorkUnpublishedResource,
        intended_terminal: ScheduledWorkPreparationTerminal,
        preparation_detail: String,
        cleanup_detail: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "generated/")]
pub enum ScheduledWorkPreparationTerminal {
    Failed,
    Cancelled,
}

impl ScheduledWorkOccurrenceState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. }
                | Self::Failed { .. }
                | Self::BudgetExceeded { .. }
                | Self::Cancelled { .. }
                | Self::PreparationFailed { .. }
                | Self::PreparationCancelled { .. }
                | Self::CleanupRequired { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ScheduledWorkOccurrence {
    pub id: ScheduledWorkOccurrenceId,
    pub scheduled_work_id: ScheduledWorkId,
    #[serde(with = "crate::wire::u64_string")]
    #[schemars(schema_with = "crate::wire::u64_string::json_schema")]
    #[ts(type = "string")]
    pub due_at_ms: u64,
    pub state: ScheduledWorkOccurrenceState,
}

/// A one-shot request whose session is supplied by the authenticated RPC
/// attachment. The caller supplies one explicit runtime selection; the daemon
/// validates it and freezes the route plus complete non-secret execution
/// request before the durable definition is created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CreateScheduledWorkRequest {
    pub objective: String,
    pub selection: AgentRuntimeSelection,
    #[serde(with = "crate::wire::u64_string")]
    #[schemars(schema_with = "crate::wire::u64_string::json_schema")]
    #[ts(type = "string")]
    pub due_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CreateScheduledWorkResult {
    pub definition: ScheduledWorkDefinition,
    pub occurrence: ScheduledWorkOccurrence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ListScheduledWorkRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct ListScheduledWorkResult {
    pub occurrences: Vec<ScheduledWorkOccurrence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "generated/")]
pub struct CancelScheduledWorkRequest {
    pub occurrence_id: ScheduledWorkOccurrenceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScheduledWorkValidationError {
    #[error("scheduled work objective must not be empty")]
    EmptyObjective,
}
