use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("{0} identifier must not be empty")]
    EmptyIdentifier(&'static str),
    #[error("approval reason must not be empty")]
    EmptyApprovalReason,
    #[error("approval actor principal id must not be empty")]
    EmptyApprovalActorPrincipalId,
    #[error("approval expiresAtMs must be greater than requestedAtMs")]
    InvalidApprovalTtl,
}
