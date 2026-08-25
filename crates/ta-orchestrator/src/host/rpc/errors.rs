use crate::{internal_error, invalid_params};

pub(super) fn map_app_service_error(
    error: crate::orchestration::AppServiceError,
) -> crate::JsonRpcErrorObject {
    match error {
        crate::orchestration::AppServiceError::EmptySessionTitle => {
            invalid_params(error.to_string())
        }
        crate::orchestration::AppServiceError::SessionWorkspaceMissing => {
            invalid_params(error.to_string())
                .with_data(serde_json::json!({ "code": "SessionWorkspaceMissing" }))
        }
        crate::orchestration::AppServiceError::WorkspaceNotFound(_) => {
            workspace_error(error.to_string(), "WorkspaceNotFound")
        }
        crate::orchestration::AppServiceError::WorkspaceNotADirectory(_) => {
            workspace_error(error.to_string(), "WorkspaceNotADirectory")
        }
        crate::orchestration::AppServiceError::WorkspaceCanonicalizeFailed { .. } => {
            workspace_error(error.to_string(), "WorkspaceCanonicalizeFailed")
        }
        crate::orchestration::AppServiceError::WorkspaceTrustRequired(_) => {
            workspace_error(error.to_string(), "WorkspaceTrustRequired")
        }
        crate::orchestration::AppServiceError::WorkspacePermissionDenied { .. } => {
            workspace_error(error.to_string(), "WorkspacePermissionDenied")
        }
        crate::orchestration::AppServiceError::WorkspaceSymlinkEscape(_) => {
            workspace_error(error.to_string(), "WorkspaceSymlinkEscape")
        }
        crate::orchestration::AppServiceError::WorkspaceOutsideAllowedRoots(_) => {
            workspace_error(error.to_string(), "WorkspaceOutsideAllowedRoots")
        }
        crate::orchestration::AppServiceError::WorkspaceCapabilityUnsupported {
            variant,
            vendor,
            capability,
            requested,
            reason,
        } => workspace_capability_error(variant, vendor, capability, requested, reason),
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
            crate::orchestration::AgentRuntimeServiceError::WorkspaceCapabilityUnsupported(
                detail,
            ) => workspace_capability_error(
                detail.variant,
                detail.vendor,
                detail.capability,
                detail.requested,
                detail.reason,
            ),
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

fn workspace_capability_error(
    variant: Option<String>,
    vendor: Option<String>,
    capability: String,
    requested: String,
    reason: String,
) -> crate::JsonRpcErrorObject {
    let message = format!("workspace capability unsupported: {reason}");
    let mut data = serde_json::json!({
        "code": "WorkspaceCapabilityUnsupported",
        "capability": capability,
        "requested": requested,
        "reason": reason,
    });
    if let Some(variant) = variant
        && let Some(object) = data.as_object_mut()
    {
        object.insert("variant".to_string(), serde_json::Value::String(variant));
    }
    if let Some(vendor) = vendor
        && let Some(object) = data.as_object_mut()
    {
        object.insert("vendor".to_string(), serde_json::Value::String(vendor));
    }
    invalid_params(message).with_data(data)
}

fn workspace_error(message: String, code: &'static str) -> crate::JsonRpcErrorObject {
    invalid_params(message).with_data(serde_json::json!({ "code": code }))
}

pub(super) fn invalid_public_approval_state() -> crate::JsonRpcErrorObject {
    invalid_params("approval is not pending")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_protocol::wire::{ValidationError, WorkspaceCapabilityUnsupported};

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

    #[test]
    fn runtime_capability_error_preserves_structured_rpc_data() {
        let error = map_app_service_error(crate::orchestration::AppServiceError::AgentRuntime(
            crate::orchestration::AgentRuntimeServiceError::WorkspaceCapabilityUnsupported(
                WorkspaceCapabilityUnsupported {
                    variant: None,
                    vendor: Some("cursor".to_string()),
                    capability: "network".to_string(),
                    requested: "none".to_string(),
                    reason: "provider cannot separate model and tool network".to_string(),
                },
            ),
        ));

        assert_eq!(error.code, crate::INVALID_PARAMS_ERROR_CODE);
        assert_eq!(
            error.data,
            Some(serde_json::json!({
                "code": "WorkspaceCapabilityUnsupported",
                "vendor": "cursor",
                "capability": "network",
                "requested": "none",
                "reason": "provider cannot separate model and tool network",
            }))
        );
    }
}
