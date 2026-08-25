use crate::orchestration::{
    AgentRuntimeServiceError, ArtifactMutationResult, RunExecutionError, RunMutationResult,
};
use crate::{ArtifactSummary, RunSummary};
use ta_protocol::wire::{OutputContractKind, RecipeResolutionError, ValidationError};
use ta_store::StoreError;
use ta_workflow::WorkflowManagerError;
use thiserror::Error;

use super::AppDeferredMutationResult;

#[derive(Debug, Error)]
pub enum AppServiceError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("session title must not be empty")]
    EmptySessionTitle,
    #[error("daemon.session.open requires a workspace selector; call daemon.workspace.open first")]
    #[allow(dead_code)]
    SessionWorkspaceMissing,
    #[error("workspace does not exist: {0}")]
    WorkspaceNotFound(String),
    #[error("workspace path is not a directory: {0}")]
    WorkspaceNotADirectory(String),
    #[error("workspace path canonicalization failed for {path}: {reason}")]
    WorkspaceCanonicalizeFailed { path: String, reason: String },
    #[error("workspace trust confirmation required: {0}")]
    WorkspaceTrustRequired(String),
    #[error("workspace permission probe failed for {path}: {reason}")]
    WorkspacePermissionDenied { path: String, reason: String },
    #[error("workspace symlink escapes allowed root: {0}")]
    #[allow(dead_code)]
    WorkspaceSymlinkEscape(String),
    #[error("workspace is outside allowed roots: {0}")]
    #[allow(dead_code)]
    WorkspaceOutsideAllowedRoots(String),
    #[error("workspace capability unsupported: {reason}")]
    #[allow(dead_code)]
    WorkspaceCapabilityUnsupported {
        variant: Option<String>,
        vendor: Option<String>,
        capability: String,
        requested: String,
        reason: String,
    },
    #[error("session owner client name must not be empty")]
    EmptySessionOwnerClientName,
    #[error("session owner principal id must not be empty")]
    EmptySessionOwnerPrincipalId,
    #[error(
        "daemon.initialize clientCredential must be at least 32 non-whitespace ASCII characters"
    )]
    InvalidClientCredentialLength,
    #[error("daemon.initialize clientCredential must not contain whitespace")]
    InvalidClientCredentialWhitespace,
    #[error("session does not exist: {0}")]
    SessionNotFound(String),
    #[error("session authority rejected: {0}")]
    SessionAuthorityRejected(String),
    #[error("activity page limit must be greater than zero")]
    InvalidActivityPageLimit,
    #[error("agent turns page limit must be greater than zero")]
    InvalidAgentTurnsPageLimit,
    #[error("native run list limit must be between 1 and {max}")]
    InvalidNativeRunListLimit { max: u32 },
    #[error("run timeline event limit must be between 1 and {max}")]
    InvalidRunTimelineLimit { max: u32 },
    #[error("native run list cursor is invalid")]
    InvalidNativeRunListCursor,
    #[error("receipt list limit must be between 1 and {max}")]
    InvalidReceiptListLimit { max: u32 },
    #[error("run objective must not be empty")]
    EmptyRunObjective,
    #[error("run does not exist: {0}")]
    RunNotFound(String),
    #[error("run does not belong to session: {0}")]
    RunSessionMismatch(String),
    #[error("run is not waiting for approval: {0}")]
    RunNotWaitingForApproval(String),
    #[error("run is not active on this runtime: {0}")]
    RunNotLiveOwned(String),
    #[error("run is not a native harness run: {0}")]
    RunNotNativeHarness(String),
    #[error("run is not resumable: {0}")]
    RunNotResumable(String),
    #[error("run fork point does not exist: {0}")]
    RunForkPointNotFound(String),
    #[error("run fork point is not a completed turn boundary: {0}")]
    RunForkPointNotTurnBoundary(String),
    #[error("run is not queued: {0}")]
    RunNotQueued(String),
    #[error("run is not cancellable: {0}")]
    RunNotCancellable(String),
    #[error("run queue is full for session: {0}")]
    RunQueueFull(String),
    #[error("output contract violation: {0}")]
    OutputContractViolation(ValidationError),
    #[error("unknown recipe id: {0}")]
    UnknownRecipeId(String),
    #[error(
        "recipe {recipe_id} requires {recipe_contract:?} output contract, got {request_contract:?}"
    )]
    RecipeContractConflict {
        recipe_id: String,
        recipe_contract: OutputContractKind,
        request_contract: OutputContractKind,
    },
    #[error("budget exceeded: {0}")]
    BudgetExceeded(String),
    #[error("approval does not exist: {0}")]
    ApprovalNotFound(String),
    #[error("approval is already resolved: {0}")]
    ApprovalAlreadyResolved(String),
    #[error("receipt does not exist: {0}")]
    ReceiptNotFound(String),
    #[error("receipt does not belong to session: {0}")]
    ReceiptSessionMismatch(String),
    #[error("work item does not exist: {0}")]
    WorkItemNotFound(String),
    #[error("background workflow is not loaded; background orchestrator is idle")]
    WorkflowNotLoaded,
    #[error(transparent)]
    Workflow(#[from] WorkflowManagerError),
    #[error("invalid receipt transition for {receipt_id}: {detail}")]
    ReceiptTransitionViolation { receipt_id: String, detail: String },
    #[error(transparent)]
    AgentRuntime(#[from] AgentRuntimeServiceError),
    #[allow(dead_code)]
    #[error("artifact storage path must not be empty")]
    EmptyArtifactStoragePath,
}

pub(super) fn map_run_execution_error(error: RunExecutionError) -> AppServiceError {
    match error {
        RunExecutionError::Store(error) => AppServiceError::Store(error),
        RunExecutionError::EmptyArtifactStoragePath => AppServiceError::EmptyArtifactStoragePath,
        RunExecutionError::EmptyRunObjective => AppServiceError::EmptyRunObjective,
        RunExecutionError::SessionNotFound(session_id) => {
            AppServiceError::SessionNotFound(session_id)
        }
        RunExecutionError::SessionWorkspaceNotFound(workspace_id) => {
            AppServiceError::WorkspaceNotFound(workspace_id)
        }
        RunExecutionError::WorkspaceTrustRequired(workspace_id) => {
            AppServiceError::WorkspaceTrustRequired(workspace_id)
        }
        RunExecutionError::WorkspaceScopeUnsupported(requested) => {
            AppServiceError::WorkspaceCapabilityUnsupported {
                variant: None,
                vendor: None,
                capability: "executionScope".to_string(),
                requested: requested.clone(),
                reason: format!("execution scope {requested} is not implemented"),
            }
        }
        RunExecutionError::WorkspaceCapabilityUnsupported(detail) => {
            AppServiceError::WorkspaceCapabilityUnsupported {
                variant: detail.variant,
                vendor: detail.vendor,
                capability: detail.capability,
                requested: detail.requested,
                reason: detail.reason,
            }
        }
        RunExecutionError::ExecutionContextPathInvalid(detail) => {
            AppServiceError::AgentRuntime(AgentRuntimeServiceError::ProviderExecutionFailed(detail))
        }
        RunExecutionError::RunNotFound(run_id) => AppServiceError::RunNotFound(run_id),
        RunExecutionError::RunSessionMismatch(run_id) => {
            AppServiceError::RunSessionMismatch(run_id)
        }
        RunExecutionError::RunNotWaitingForApproval(run_id) => {
            AppServiceError::RunNotWaitingForApproval(run_id)
        }
        RunExecutionError::RunNotLiveOwned(run_id) => AppServiceError::RunNotLiveOwned(run_id),
        RunExecutionError::RunNotNativeHarness(run_id) => {
            AppServiceError::RunNotNativeHarness(run_id)
        }
        RunExecutionError::RunNotResumable(run_id) => AppServiceError::RunNotResumable(run_id),
        RunExecutionError::RunForkPointNotFound(run_id) => {
            AppServiceError::RunForkPointNotFound(run_id)
        }
        RunExecutionError::RunForkPointNotTurnBoundary(run_id) => {
            AppServiceError::RunForkPointNotTurnBoundary(run_id)
        }
        RunExecutionError::RunNotQueued(run_id) => AppServiceError::RunNotQueued(run_id),
        RunExecutionError::RunNotCancellable(run_id) => AppServiceError::RunNotCancellable(run_id),
        RunExecutionError::RunQueueFull(session_id) => AppServiceError::RunQueueFull(session_id),
        RunExecutionError::OutputContractViolation(detail) => {
            AppServiceError::OutputContractViolation(detail)
        }
        RunExecutionError::UnknownRecipeId(recipe_id) => {
            AppServiceError::UnknownRecipeId(recipe_id)
        }
        RunExecutionError::RecipeContractConflict {
            recipe_id,
            recipe_contract,
            request_contract,
        } => AppServiceError::RecipeContractConflict {
            recipe_id,
            recipe_contract,
            request_contract,
        },
        RunExecutionError::BudgetExceeded(detail) => AppServiceError::BudgetExceeded(detail),
        RunExecutionError::ProviderExecutionFailed(error) => {
            AppServiceError::AgentRuntime(AgentRuntimeServiceError::ProviderExecutionFailed(error))
        }
        RunExecutionError::ApprovalNotFound(approval_id) => {
            AppServiceError::ApprovalNotFound(approval_id)
        }
        RunExecutionError::ApprovalAlreadyResolved(approval_id) => {
            AppServiceError::ApprovalAlreadyResolved(approval_id)
        }
    }
}

pub(crate) fn recipe_resolution_error_data(
    error: &AppServiceError,
) -> Option<RecipeResolutionError> {
    match error {
        AppServiceError::UnknownRecipeId(recipe_id) => {
            Some(RecipeResolutionError::UnknownRecipeId {
                recipe_id: recipe_id.clone(),
            })
        }
        AppServiceError::RecipeContractConflict {
            recipe_id,
            recipe_contract,
            request_contract,
        } => Some(RecipeResolutionError::RecipeContractConflict {
            recipe_id: recipe_id.clone(),
            recipe_contract: *recipe_contract,
            request_contract: *request_contract,
        }),
        _ => None,
    }
}

pub(super) fn map_receipt_store_error(error: StoreError) -> AppServiceError {
    match error {
        StoreError::ReceiptTransitionViolation { receipt_id, detail } => {
            AppServiceError::ReceiptTransitionViolation { receipt_id, detail }
        }
        error => AppServiceError::Store(error),
    }
}

pub(super) fn map_run_mutation_result(
    result: RunMutationResult,
) -> AppDeferredMutationResult<RunSummary> {
    AppDeferredMutationResult {
        body: result.run,
        deferred_records: result.events,
    }
}

pub(super) fn map_artifact_mutation_result(
    result: ArtifactMutationResult,
) -> AppDeferredMutationResult<ArtifactSummary> {
    AppDeferredMutationResult {
        body: result.artifact,
        deferred_records: result.events,
    }
}
