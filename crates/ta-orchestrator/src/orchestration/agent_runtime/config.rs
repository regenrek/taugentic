use ta_protocol::wire::{
    RuntimePolicyMode, RuntimeProfileExecutionKind, RuntimeProfilePatch, RuntimeProfileSummary,
};

use crate::orchestration::agent_runtime::{
    service::AgentRuntimeServiceError, strategy_registry::StrategyRegistry,
};

pub(crate) fn apply_runtime_profile_patch(
    profiles: &mut [RuntimeProfileSummary],
    registry: &StrategyRegistry,
    runtime_profile_id: &ta_protocol::wire::RuntimeProfileId,
    patch: &RuntimeProfilePatch,
) -> Result<RuntimeProfileSummary, AgentRuntimeServiceError> {
    let index = profiles
        .iter()
        .position(|profile| profile.id == *runtime_profile_id)
        .ok_or_else(|| {
            AgentRuntimeServiceError::RuntimeProfileNotFound(
                runtime_profile_id.as_str().to_string(),
            )
        })?;

    let mut updated = profiles[index].clone();
    if let Some(display_name) = patch.display_name.as_deref() {
        let trimmed = display_name.trim();
        if trimmed.is_empty() {
            return Err(AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                "runtime profile display name must not be empty".to_string(),
            ));
        }
        updated.display_name = trimmed.to_string();
    }
    if let Some(policy_mode) = patch.policy_mode {
        if updated.execution_kind == RuntimeProfileExecutionKind::RealtimeVoice
            && policy_mode != RuntimePolicyMode::Deny
        {
            return Err(AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
                "realtime voice profiles must deny agent approval dispatch".to_string(),
            ));
        }
        updated.policy_mode = policy_mode;
    }

    validate_runtime_profile(&updated, registry)?;
    profiles[index] = updated.clone();
    Ok(updated)
}

pub(crate) fn validate_runtime_profile(
    profile: &RuntimeProfileSummary,
    registry: &StrategyRegistry,
) -> Result<RuntimeProfileSummary, AgentRuntimeServiceError> {
    validate_provider_exists(registry, &profile.provider_id)?;
    if profile.execution_kind == RuntimeProfileExecutionKind::RealtimeVoice
        && profile.policy_mode != RuntimePolicyMode::Deny
    {
        return Err(AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
            "realtime voice profiles must deny agent approval dispatch".to_string(),
        ));
    }
    Ok(profile.clone())
}

fn validate_provider_exists(
    registry: &StrategyRegistry,
    provider_id: &ta_protocol::wire::AgentRuntimeStrategyId,
) -> Result<(), AgentRuntimeServiceError> {
    if registry.contains_provider(provider_id) {
        return Ok(());
    }
    Err(AgentRuntimeServiceError::InvalidAgentRuntimeConfig(
        format!(
            "runtime profile references unknown provider {}",
            provider_id.as_str(),
        ),
    ))
}
