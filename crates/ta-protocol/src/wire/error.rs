use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("{0} identifier must not be empty")]
    EmptyIdentifier(&'static str),
    #[error("approval reason must not be empty")]
    EmptyApprovalReason,
    #[error("approval actor principal id must not be empty")]
    EmptyApprovalActorPrincipalId,
    #[error("run status reason must not be empty")]
    EmptyRunStatusReason,
    #[error("run status must be active")]
    RunStatusMustBeActive,
    #[error("run status must be terminal")]
    RunStatusMustBeTerminal,
    #[error("active run status must not include a reason")]
    ActiveRunStatusHasReason,
    #[error("terminal run status requires a reason")]
    TerminalRunStatusMissingReason,
    #[error("approval expiresAtMs must be greater than requestedAtMs")]
    InvalidApprovalTtl,
}
