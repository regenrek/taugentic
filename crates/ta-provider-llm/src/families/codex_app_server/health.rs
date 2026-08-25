use crate::error::LlmClientError;
use ta_protocol::wire::{
    AgentRuntimeModelAvailability, AgentRuntimeModelCapability, AgentRuntimeStrategyHealth,
    AgentRuntimeStrategyHealthStatus, AgentRuntimeStrategyId, AgentRuntimeStrategyInfo,
    AuthProfileId,
};

use super::client::CodexCli;
use super::{CODEX_PROVIDER_ID, CodexAuthMode, CodexProviderSnapshot, model_catalog};
use crate::auth::codex_oauth::{auth_mode, auth_profiles_for_mode};

pub fn snapshot() -> Result<CodexProviderSnapshot, LlmClientError> {
    let cli = CodexCli::default();
    let mode = auth_mode(&cli);
    let catalog = model_catalog()?;
    Ok(CodexProviderSnapshot {
        provider: AgentRuntimeStrategyInfo {
            id: AgentRuntimeStrategyId::new(CODEX_PROVIDER_ID).expect("provider id"),
            display_name: "Codex".to_string(),
            models: catalog.models,
            model_capability: AgentRuntimeModelCapability {
                availability: AgentRuntimeModelAvailability::Enumerated,
                can_set_model: true,
                current_model_id: catalog.default_model_id,
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
    fn provider_health_keeps_authenticated_codex_ready() {
        assert_eq!(
            provider_health(&CodexAuthMode::Chatgpt).status,
            AgentRuntimeStrategyHealthStatus::Ready
        );
        assert_ne!(CODEX_CHATGPT_AUTH_PROFILE_ID, CODEX_API_KEY_AUTH_PROFILE_ID);
    }
}
