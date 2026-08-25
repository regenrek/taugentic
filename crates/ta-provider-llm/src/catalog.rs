use std::sync::OnceLock;

use serde::Deserialize;
use ta_protocol::wire::{
    AgentRuntimeModelId, AgentRuntimeModelRef, AgentRuntimeStrategyId, AuthProfileId,
    RuntimePolicyMode, RuntimeProfileId, RuntimeProfileSummary,
};

use crate::families::codex_app_server::{CODEX_CHATGPT_AUTH_PROFILE_ID, CODEX_PROVIDER_ID};
use crate::families::{
    anthropic::{
        ANTHROPIC_API_KEY_AUTH_PROFILE_ID, ANTHROPIC_DEFAULT_MODEL_ID, ANTHROPIC_PROVIDER_ID,
    },
    openai::{
        OPENAI_API_KEY_AUTH_PROFILE_ID, OPENAI_CHATGPT_AUTH_PROFILE_ID, OPENAI_DEFAULT_MODEL_ID,
        OPENAI_PROVIDER_ID,
    },
};

const OPENAI_MODEL_CATALOG_JSON: &str = include_str!("catalog/openai.json");
const ANTHROPIC_MODEL_CATALOG_JSON: &str = include_str!("catalog/anthropic.json");

static OPENAI_MODEL_CATALOG: OnceLock<Vec<OpenAiCatalogModel>> = OnceLock::new();
static ANTHROPIC_MODEL_CATALOG: OnceLock<Vec<CatalogModel>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogModel {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum OpenAiWireApi {
    Responses,
    ChatCompletions,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenAiCatalogModel {
    pub id: String,
    pub display_name: String,
    pub wire_api: OpenAiWireApi,
}

pub fn openai_models() -> Vec<AgentRuntimeModelRef> {
    openai_model_catalog()
        .iter()
        .map(|model| AgentRuntimeModelRef {
            id: model_id(&model.id),
            display_name: model.display_name.clone(),
            context_limit: None,
            input_token_cost_micros: None,
            output_token_cost_micros: None,
        })
        .collect()
}

pub fn anthropic_models() -> Vec<AgentRuntimeModelRef> {
    anthropic_model_catalog()
        .iter()
        .map(|model| AgentRuntimeModelRef {
            id: model_id(&model.id),
            display_name: model.display_name.clone(),
            context_limit: None,
            input_token_cost_micros: None,
            output_token_cost_micros: None,
        })
        .collect()
}

pub fn openai_wire_api(model_id: &AgentRuntimeModelId) -> Result<OpenAiWireApi, String> {
    openai_model_catalog()
        .iter()
        .find(|model| model.id == model_id.as_str())
        .map(|model| model.wire_api)
        .ok_or_else(|| format!("unknown OpenAI model {}", model_id.as_str()))
}

pub fn codex_default_runtime_profiles() -> Vec<RuntimeProfileSummary> {
    let provider_id = AgentRuntimeStrategyId::new(CODEX_PROVIDER_ID).expect("provider id");
    let auth_profile_id =
        AuthProfileId::new(CODEX_CHATGPT_AUTH_PROFILE_ID).expect("auth profile id");
    vec![
        RuntimeProfileSummary {
            id: RuntimeProfileId::new("runtime-codex-safe").expect("runtime profile id"),
            display_name: "Codex Safe".to_string(),
            provider_id: provider_id.clone(),
            model_id: None,
            auth_profile_id: Some(auth_profile_id.clone()),
            policy_mode: RuntimePolicyMode::RequireApproval,
        },
        RuntimeProfileSummary {
            id: RuntimeProfileId::new("runtime-codex-allow").expect("runtime profile id"),
            display_name: "Codex Allow".to_string(),
            provider_id: provider_id.clone(),
            model_id: None,
            auth_profile_id: Some(auth_profile_id.clone()),
            policy_mode: RuntimePolicyMode::Allow,
        },
        RuntimeProfileSummary {
            id: RuntimeProfileId::new("runtime-codex-deny").expect("runtime profile id"),
            display_name: "Codex Deny".to_string(),
            provider_id,
            model_id: None,
            auth_profile_id: Some(auth_profile_id),
            policy_mode: RuntimePolicyMode::Deny,
        },
    ]
}

pub fn openai_default_runtime_profiles() -> Vec<RuntimeProfileSummary> {
    let mut profiles = default_runtime_profiles(
        OPENAI_PROVIDER_ID,
        OPENAI_API_KEY_AUTH_PROFILE_ID,
        OPENAI_DEFAULT_MODEL_ID,
        "OpenAI",
    );
    profiles.extend(default_runtime_profiles_with_id_prefix(
        OPENAI_PROVIDER_ID,
        OPENAI_CHATGPT_AUTH_PROFILE_ID,
        OPENAI_DEFAULT_MODEL_ID,
        "runtime-openai-chatgpt",
        "OpenAI ChatGPT",
    ));
    profiles
}

pub fn anthropic_default_runtime_profiles() -> Vec<RuntimeProfileSummary> {
    default_runtime_profiles(
        ANTHROPIC_PROVIDER_ID,
        ANTHROPIC_API_KEY_AUTH_PROFILE_ID,
        ANTHROPIC_DEFAULT_MODEL_ID,
        "Anthropic",
    )
}

fn default_runtime_profiles(
    provider_id: &str,
    auth_profile_id: &str,
    default_model_id: &str,
    display_prefix: &str,
) -> Vec<RuntimeProfileSummary> {
    default_runtime_profiles_with_id_prefix(
        provider_id,
        auth_profile_id,
        default_model_id,
        &format!("runtime-{provider_id}"),
        display_prefix,
    )
}

fn default_runtime_profiles_with_id_prefix(
    provider_id: &str,
    auth_profile_id: &str,
    default_model_id: &str,
    runtime_id_prefix: &str,
    display_prefix: &str,
) -> Vec<RuntimeProfileSummary> {
    let provider_id = AgentRuntimeStrategyId::new(provider_id).expect("provider id");
    let auth_profile_id = AuthProfileId::new(auth_profile_id).expect("auth profile id");
    let default_model_id = model_id(default_model_id);
    [
        (
            "safe",
            "Safe",
            RuntimePolicyMode::RequireApproval,
            provider_id.clone(),
        ),
        (
            "allow",
            "Allow",
            RuntimePolicyMode::Allow,
            provider_id.clone(),
        ),
        ("deny", "Deny", RuntimePolicyMode::Deny, provider_id),
    ]
    .into_iter()
    .map(
        |(suffix, label, policy_mode, provider_id)| RuntimeProfileSummary {
            id: RuntimeProfileId::new(format!("{runtime_id_prefix}-{suffix}"))
                .expect("runtime profile id"),
            display_name: format!("{display_prefix} {label}"),
            provider_id,
            model_id: Some(default_model_id.clone()),
            auth_profile_id: Some(auth_profile_id.clone()),
            policy_mode,
        },
    )
    .collect()
}

fn model_id(value: &str) -> AgentRuntimeModelId {
    AgentRuntimeModelId::new(value).expect("model id")
}

fn openai_model_catalog() -> &'static [OpenAiCatalogModel] {
    OPENAI_MODEL_CATALOG.get_or_init(|| {
        serde_json::from_str(OPENAI_MODEL_CATALOG_JSON)
            .expect("embedded OpenAI model catalog JSON must stay valid")
    })
}

fn anthropic_model_catalog() -> &'static [CatalogModel] {
    ANTHROPIC_MODEL_CATALOG.get_or_init(|| {
        serde_json::from_str(ANTHROPIC_MODEL_CATALOG_JSON)
            .expect("embedded Anthropic model catalog JSON must stay valid")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_openai_and_anthropic_catalog_json_parse_into_models() {
        assert!(!openai_model_catalog().is_empty());
        assert!(!anthropic_model_catalog().is_empty());
    }

    #[test]
    fn codex_default_runtime_profiles_delegate_the_default_model_to_codex() {
        assert!(
            codex_default_runtime_profiles()
                .iter()
                .all(|profile| profile.model_id.is_none())
        );
    }

    #[test]
    fn native_default_runtime_profiles_pin_existing_catalog_models() {
        let openai_ids = openai_model_catalog()
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>();
        for profile in openai_default_runtime_profiles() {
            assert!(openai_ids.iter().any(
                |candidate| Some(*candidate) == profile.model_id.as_ref().map(|id| id.as_str())
            ));
        }

        let anthropic_ids = anthropic_model_catalog()
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>();
        for profile in anthropic_default_runtime_profiles() {
            assert!(anthropic_ids.iter().any(
                |candidate| Some(*candidate) == profile.model_id.as_ref().map(|id| id.as_str())
            ));
        }
    }

    #[test]
    fn openai_default_runtime_profiles_include_api_key_and_chatgpt_variants() {
        let profiles = openai_default_runtime_profiles();

        assert_eq!(profiles.len(), 6);
        for (profile_id, display_name, policy_mode) in [
            (
                "runtime-openai-safe",
                "OpenAI Safe",
                RuntimePolicyMode::RequireApproval,
            ),
            (
                "runtime-openai-allow",
                "OpenAI Allow",
                RuntimePolicyMode::Allow,
            ),
            (
                "runtime-openai-deny",
                "OpenAI Deny",
                RuntimePolicyMode::Deny,
            ),
        ] {
            let profile = profiles
                .iter()
                .find(|profile| profile.id.as_str() == profile_id)
                .expect("api key OpenAI runtime profile");
            assert_eq!(profile.display_name, display_name);
            assert_eq!(
                profile.auth_profile_id.as_ref().map(|id| id.as_str()),
                Some(OPENAI_API_KEY_AUTH_PROFILE_ID)
            );
            assert_eq!(profile.provider_id.as_str(), OPENAI_PROVIDER_ID);
            assert_eq!(
                profile.model_id.as_ref().map(|id| id.as_str()),
                Some(OPENAI_DEFAULT_MODEL_ID)
            );
            assert_eq!(profile.policy_mode, policy_mode);
        }

        for (profile_id, display_name, policy_mode) in [
            (
                "runtime-openai-chatgpt-safe",
                "OpenAI ChatGPT Safe",
                RuntimePolicyMode::RequireApproval,
            ),
            (
                "runtime-openai-chatgpt-allow",
                "OpenAI ChatGPT Allow",
                RuntimePolicyMode::Allow,
            ),
            (
                "runtime-openai-chatgpt-deny",
                "OpenAI ChatGPT Deny",
                RuntimePolicyMode::Deny,
            ),
        ] {
            let profile = profiles
                .iter()
                .find(|profile| profile.id.as_str() == profile_id)
                .expect("ChatGPT OpenAI runtime profile");
            assert_eq!(profile.display_name, display_name);
            assert_eq!(
                profile.auth_profile_id.as_ref().map(|id| id.as_str()),
                Some(OPENAI_CHATGPT_AUTH_PROFILE_ID)
            );
            assert_eq!(profile.provider_id.as_str(), OPENAI_PROVIDER_ID);
            assert_eq!(
                profile.model_id.as_ref().map(|id| id.as_str()),
                Some(OPENAI_DEFAULT_MODEL_ID)
            );
            assert_eq!(profile.policy_mode, policy_mode);
        }
    }
}
