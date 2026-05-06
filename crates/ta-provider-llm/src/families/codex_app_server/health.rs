use crate::error::LlmClientError;
use ta_protocol::wire::{
    AgentRuntimeModelAvailability, AgentRuntimeModelCapability, AgentRuntimeStrategyHealth,
    AgentRuntimeStrategyHealthStatus, AgentRuntimeStrategyId, AgentRuntimeStrategyInfo,
    AuthProfileId,
};

use super::client::CodexCli;
use super::{CODEX_PROVIDER_ID, CodexAuthMode, CodexProviderSnapshot};
use crate::auth::codex_oauth::{auth_mode, auth_profiles_for_mode};
use crate::catalog::codex_models;

pub fn snapshot() -> Result<CodexProviderSnapshot, LlmClientError> {
    let cli = CodexCli::default();
    let mode = auth_mode(&cli);
    Ok(CodexProviderSnapshot {
        provider: AgentRuntimeStrategyInfo {
            id: AgentRuntimeStrategyId::new(CODEX_PROVIDER_ID).expect("provider id"),
            display_name: "Codex".to_string(),
            models: codex_models(),
            model_capability: AgentRuntimeModelCapability {
                availability: AgentRuntimeModelAvailability::Enumerated,
                can_set_model: true,
                current_model_id: None,
                detail: None,
            },
            health: provider_health(&mode),
        },
        auth_profiles: auth_profiles_for_mode(&mode),
    })
}

pub fn login(auth_profile_id: &AuthProfileId) -> super::CodexLoginResult {
    crate::auth::codex_oauth::login(&CodexCli::default(), auth_profile_id)
}

pub fn logout(auth_profile_id: &AuthProfileId) -> super::CodexLogoutResult {
    crate::auth::codex_oauth::logout(&CodexCli::default(), auth_profile_id)
}

fn provider_health(mode: &CodexAuthMode) -> AgentRuntimeStrategyHealth {
    match mode {
        CodexAuthMode::Chatgpt => AgentRuntimeStrategyHealth {
            status: AgentRuntimeStrategyHealthStatus::Ready,
            message: Some("codex CLI authenticated with ChatGPT".to_string()),
        },
        CodexAuthMode::ApiKey => AgentRuntimeStrategyHealth {
            status: AgentRuntimeStrategyHealthStatus::Ready,
            message: Some("codex CLI authenticated with API key".to_string()),
        },
        CodexAuthMode::LoggedOut => AgentRuntimeStrategyHealth {
            status: AgentRuntimeStrategyHealthStatus::Degraded,
            message: Some("codex CLI is installed but not authenticated".to_string()),
        },
        CodexAuthMode::Unknown(message) => AgentRuntimeStrategyHealth {
            status: AgentRuntimeStrategyHealthStatus::Degraded,
            message: Some(format!(
                "codex CLI returned unexpected auth status: {message}"
            )),
        },
        CodexAuthMode::Unavailable(message) => AgentRuntimeStrategyHealth {
            status: AgentRuntimeStrategyHealthStatus::Unavailable,
            message: Some(message.clone()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::codex_app_server::{
        CODEX_API_KEY_AUTH_PROFILE_ID, CODEX_CHATGPT_AUTH_PROFILE_ID,
    };

    #[test]
    fn snapshot_exposes_stable_codex_auth_profiles() {
        let snapshot = snapshot().expect("snapshot should not fail");
        let auth_ids = snapshot
            .auth_profiles
            .iter()
            .map(|profile| profile.profile.id.as_str())
            .collect::<Vec<_>>();

        assert!(auth_ids.contains(&CODEX_CHATGPT_AUTH_PROFILE_ID));
        assert!(auth_ids.contains(&CODEX_API_KEY_AUTH_PROFILE_ID));
    }
}
