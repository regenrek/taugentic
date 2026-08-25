use ta_protocol::wire::{
    AgentRuntimeModelId, AuthProfileId, RuntimeProfileAuthProfilePatch, RuntimeProfileModelIdPatch,
    RuntimeProfilePatch, RuntimeProfileSummary,
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
    if let Some(provider_id) = patch.provider_id.as_ref() {
        updated.provider_id = provider_id.clone();
    }
    if let Some(model_patch) = patch.model_id.as_ref() {
        updated.model_id = apply_model_patch(model_patch);
    }
    if let Some(auth_patch) = patch.auth_profile.as_ref() {
        updated.auth_profile_id = apply_auth_patch(auth_patch);
    }
    if let Some(policy_mode) = patch.policy_mode {
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
    if let Some(model_id) = profile.model_id.as_ref()
        && !registry.has_model(&profile.provider_id, model_id)
    {
        return Err(AgentRuntimeServiceError::UnknownModel {
            provider_id: profile.provider_id.as_str().to_string(),
            model_id: model_id.as_str().to_string(),
        });
    }
    if let Some(auth_profile_id) = profile.auth_profile_id.as_ref() {
        let auth_profile_matches_provider = registry
            .auth_profile_ref(auth_profile_id)
            .is_some_and(|auth_profile| auth_profile.provider_id == profile.provider_id);
        if !auth_profile_matches_provider {
            return Err(AgentRuntimeServiceError::UnknownAuthProfile {
                provider_id: profile.provider_id.as_str().to_string(),
                auth_profile_id: auth_profile_id.as_str().to_string(),
            });
        }
    }
    Ok(profile.clone())
}

fn apply_model_patch(model_patch: &RuntimeProfileModelIdPatch) -> Option<AgentRuntimeModelId> {
    match model_patch {
        RuntimeProfileModelIdPatch::Set { value } => Some(value.clone()),
        RuntimeProfileModelIdPatch::Clear => None,
    }
}

fn apply_auth_patch(auth_patch: &RuntimeProfileAuthProfilePatch) -> Option<AuthProfileId> {
    match auth_patch {
        RuntimeProfileAuthProfilePatch::Set { value } => Some(value.clone()),
        RuntimeProfileAuthProfilePatch::Clear => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::agent_runtime::strategy_registry::strategy_descriptor;
    use ta_protocol::wire::{
        AgentRuntimeStrategyId, AuthProfileRef, RuntimePolicyMode, RuntimeProfileId,
    };

    fn registry() -> StrategyRegistry {
        let provider_id = strategy_id("openai");
        StrategyRegistry::new(vec![strategy_descriptor(
            provider_id.clone(),
            "OpenAI",
            vec![AuthProfileRef {
                id: auth_profile_id("auth-a"),
                provider_id: provider_id.clone(),
                display_name: "Auth A".to_string(),
            }],
            vec![runtime_profile(
                "runtime-a",
                provider_id,
                Some("gpt-5.6-sol"),
                Some("auth-a"),
                RuntimePolicyMode::Allow,
            )],
        )])
        .expect("registry")
    }

    fn runtime_profile(
        id: &str,
        provider_id: AgentRuntimeStrategyId,
        model_id: Option<&str>,
        auth_profile_id_value: Option<&str>,
        policy_mode: RuntimePolicyMode,
    ) -> RuntimeProfileSummary {
        RuntimeProfileSummary {
            id: RuntimeProfileId::new(id).expect("runtime profile id"),
            display_name: "Runtime A".to_string(),
            provider_id,
            model_id: model_id.map(model_id_value),
            auth_profile_id: auth_profile_id_value.map(auth_profile_id),
            policy_mode,
        }
    }

    fn strategy_id(value: &str) -> AgentRuntimeStrategyId {
        AgentRuntimeStrategyId::new(value).expect("provider id")
    }

    fn model_id_value(value: &str) -> AgentRuntimeModelId {
        AgentRuntimeModelId::new(value).expect("model id")
    }

    fn auth_profile_id(value: &str) -> AuthProfileId {
        AuthProfileId::new(value).expect("auth profile id")
    }

    fn patch_model(value: &str) -> RuntimeProfilePatch {
        RuntimeProfilePatch {
            model_id: Some(RuntimeProfileModelIdPatch::Set {
                value: model_id_value(value),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn explicit_model_patch_validation_is_strict() {
        let registry = registry();

        for (case, patch, expected_model, should_reject) in [
            (
                "invalid explicit model",
                patch_model("missing-model"),
                None,
                true,
            ),
            (
                "valid explicit model",
                patch_model("gpt-5.6-terra"),
                Some(model_id_value("gpt-5.6-terra")),
                false,
            ),
        ] {
            let mut profiles = vec![runtime_profile(
                "runtime-a",
                strategy_id("openai"),
                Some("gpt-5.6-sol"),
                Some("auth-a"),
                RuntimePolicyMode::Allow,
            )];
            let result = apply_runtime_profile_patch(
                &mut profiles,
                &registry,
                &RuntimeProfileId::new("runtime-a").expect("runtime profile id"),
                &patch,
            );

            if should_reject {
                assert!(
                    matches!(
                        result,
                        Err(AgentRuntimeServiceError::UnknownModel {
                            provider_id,
                            model_id
                        }) if provider_id == "openai" && model_id == "missing-model"
                    ),
                    "{case} should reject with UnknownModel",
                );
            } else {
                let updated = result.unwrap_or_else(|error| panic!("{case} failed: {error}"));
                assert_eq!(updated.model_id, expected_model);
            }
        }
    }

    #[test]
    fn explicit_auth_patch_rejects_unknown_auth_profile_and_allows_clear() {
        let registry = registry();
        let runtime_profile_id = RuntimeProfileId::new("runtime-a").expect("runtime profile id");
        let mut profiles = vec![runtime_profile(
            "runtime-a",
            strategy_id("openai"),
            Some("gpt-5.6-sol"),
            Some("auth-a"),
            RuntimePolicyMode::Allow,
        )];

        let error = apply_runtime_profile_patch(
            &mut profiles,
            &registry,
            &runtime_profile_id,
            &RuntimeProfilePatch {
                auth_profile: Some(RuntimeProfileAuthProfilePatch::Set {
                    value: auth_profile_id("missing-auth"),
                }),
                ..Default::default()
            },
        )
        .expect_err("unknown explicit auth profile should reject");
        assert!(matches!(
            error,
            AgentRuntimeServiceError::UnknownAuthProfile {
                provider_id,
                auth_profile_id
            } if provider_id == "openai" && auth_profile_id == "missing-auth"
        ));

        let updated = apply_runtime_profile_patch(
            &mut profiles,
            &registry,
            &runtime_profile_id,
            &RuntimeProfilePatch {
                auth_profile: Some(RuntimeProfileAuthProfilePatch::Clear),
                ..Default::default()
            },
        )
        .expect("explicit auth clear should succeed");
        assert_eq!(updated.auth_profile_id, None);
    }
}
