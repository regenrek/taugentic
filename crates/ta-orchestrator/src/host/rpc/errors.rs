use crate::{internal_error, invalid_params};

pub(super) fn map_app_service_error(
    error: crate::orchestration::AppServiceError,
) -> crate::JsonRpcErrorObject {
    match error {
        crate::orchestration::AppServiceError::EmptySessionTitle => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::EmptySessionOwnerClientName => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::EmptySessionOwnerPrincipalId
        | crate::orchestration::AppServiceError::InvalidClientCredentialLength
        | crate::orchestration::AppServiceError::InvalidClientCredentialWhitespace
        | crate::orchestration::AppServiceError::InvalidActivityPageLimit
        | crate::orchestration::AppServiceError::InvalidAgentTurnsPageLimit
        | crate::orchestration::AppServiceError::InvalidNativeRunListLimit { .. }
        | crate::orchestration::AppServiceError::InvalidRunTimelineLimit { .. }
        | crate::orchestration::AppServiceError::InvalidNativeRunListCursor
        | crate::orchestration::AppServiceError::InvalidReceiptListLimit { .. } => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::EmptyRunObjective => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::RunNotFound(_) => invalid_params(error.to_string()),
        crate::orchestration::AppServiceError::RunSessionMismatch(_) => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::RunNotWaitingForApproval(_) => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::RunNotLiveOwned(_) => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::RunNotNativeHarness(_) => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::RunNotResumable(_) => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::RunForkPointNotFound(_) => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::RunForkPointNotTurnBoundary(_) => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::RunNotQueued(_) => internal_error(error.to_string()),
        crate::orchestration::AppServiceError::RunNotCancellable(_) => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::OutputContractViolation(validation_error) => {
            invalid_params(format!("output contract violation: {validation_error}")).with_data(
                serde_json::to_value(validation_error)
                    .expect("ValidationError should serialize for RPC error data"),
            )
        }
        crate::orchestration::AppServiceError::UnknownRecipeId(_)
        | crate::orchestration::AppServiceError::RecipeContractConflict { .. } => {
            let mut error_object = invalid_params(error.to_string());
            if let Some(data) = crate::orchestration::recipe_resolution_error_data(&error) {
                error_object = error_object.with_data(
                    serde_json::to_value(data)
                        .expect("RecipeResolutionError should serialize for RPC error data"),
                );
            }
            error_object
        }
        crate::orchestration::AppServiceError::BudgetExceeded(_) => {
            internal_error(error.to_string())
        }
        crate::orchestration::AppServiceError::RunQueueFull(_) => internal_error(error.to_string()),
        crate::orchestration::AppServiceError::ApprovalNotFound(_)
        | crate::orchestration::AppServiceError::ApprovalAlreadyResolved(_) => {
            invalid_public_approval_state()
        }
        crate::orchestration::AppServiceError::ReceiptNotFound(_)
        | crate::orchestration::AppServiceError::ReceiptSessionMismatch(_)
        | crate::orchestration::AppServiceError::ReceiptTransitionViolation { .. } => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::WorkItemNotFound(_) => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::WorkflowNotLoaded
        | crate::orchestration::AppServiceError::Workflow(_) => invalid_params(error.to_string()),
        crate::orchestration::AppServiceError::EmptyArtifactStoragePath => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::AgentRuntime(error) => match error {
            crate::orchestration::AgentRuntimeServiceError::NoRuntimeProviderConfigured
            | crate::orchestration::AgentRuntimeServiceError::RuntimeProfileNotFound(_)
            | crate::orchestration::AgentRuntimeServiceError::AuthProfileNotFound(_)
            | crate::orchestration::AgentRuntimeServiceError::UnknownModel { .. }
            | crate::orchestration::AgentRuntimeServiceError::UnknownAuthProfile { .. }
            | crate::orchestration::AgentRuntimeServiceError::RuntimeExtensionNotFound(_)
            | crate::orchestration::AgentRuntimeServiceError::InvalidAgentRuntimeConfig(_) => {
                invalid_params(error.to_string())
            }
            crate::orchestration::AgentRuntimeServiceError::ProviderExecutionFailed(_) => {
                internal_error(error.to_string())
            }
        },
        crate::orchestration::AppServiceError::SessionNotFound(session_id) => {
            invalid_params(format!("session does not exist: {session_id}"))
        }
        crate::orchestration::AppServiceError::SessionAuthorityRejected(session_id) => {
            invalid_params(format!("session authority rejected: {session_id}"))
        }
        crate::orchestration::AppServiceError::Store(error) => internal_error(error.to_string()),
    }
}

pub(super) fn invalid_public_approval_state() -> crate::JsonRpcErrorObject {
    invalid_params("approval is not pending")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_protocol::wire::ValidationError;

    #[test]
    fn output_contract_violation_preserves_structured_validation_error() {
        let error = map_app_service_error(
            crate::orchestration::AppServiceError::OutputContractViolation(
                ValidationError::ConfidenceOutOfRange { value: 1.5 },
            ),
        );

        assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
        assert_eq!(
            error.data,
            Some(serde_json::json!({
                "kind": "confidenceOutOfRange",
                "value": {
                    "value": 1.5
                }
            }))
        );
    }
}
