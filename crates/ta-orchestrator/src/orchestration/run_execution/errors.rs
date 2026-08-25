use ta_protocol::wire::{OutputContractKind, ValidationError, WorkspaceCapabilityUnsupported};
use ta_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunExecutionError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("artifact storage path must not be empty")]
    EmptyArtifactStoragePath,
    #[error("run objective must not be empty")]
    EmptyRunObjective,
    #[error("session does not exist: {0}")]
    SessionNotFound(String),
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
    #[cfg_attr(not(test), allow(dead_code))]
    #[error("run is not cancellable: {0}")]
    RunNotCancellable(String),
    #[error("run queue is full for session: {0}")]
    RunQueueFull(String),
    #[error("session workspace does not exist: {0}")]
    SessionWorkspaceNotFound(String),
    #[error("workspace trust confirmation is required: {0}")]
    WorkspaceTrustRequired(String),
    #[error("workspace execution scope is not supported: {0}")]
    WorkspaceScopeUnsupported(String),
    #[error("{0}")]
    WorkspaceCapabilityUnsupported(WorkspaceCapabilityUnsupported),
    #[error("execution context path is invalid: {0}")]
    ExecutionContextPathInvalid(String),
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
    #[error("output contract violation: {0}")]
    OutputContractViolation(ValidationError),
    #[error("budget exceeded: {0}")]
    BudgetExceeded(String),
    #[error("approval does not exist: {0}")]
    ApprovalNotFound(String),
    #[error("approval is already resolved: {0}")]
    ApprovalAlreadyResolved(String),
    #[error("{0}")]
    ProviderExecutionFailed(String),
}

pub(super) fn map_agent_runtime_error(error: crate::AgentRuntimeServiceError) -> RunExecutionError {
    match error {
        crate::AgentRuntimeServiceError::WorkspaceCapabilityUnsupported(detail) => {
            RunExecutionError::WorkspaceCapabilityUnsupported(detail)
        }
        error => RunExecutionError::ProviderExecutionFailed(error.to_string()),
    }
}
