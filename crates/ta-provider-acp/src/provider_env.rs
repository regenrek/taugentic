use std::collections::HashMap;

use crate::descriptor::{AcpLaunchKind, AcpProviderSpec};

pub const ANTHROPIC_API_KEY_ENV: &str = "ANTHROPIC_API_KEY";
pub const GEMINI_API_KEY_ENV: &str = "GEMINI_API_KEY";
pub const OPENAI_API_KEY_ENV: &str = "OPENAI_API_KEY";
pub const OPENCODE_API_KEY_ENV: &str = "OPENCODE_API_KEY";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProviderEnv {
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
    pub opencode_api_key: Option<String>,
    pub extra: HashMap<String, String>,
}

impl ProviderEnv {
    pub fn from_process_env() -> Self {
        Self {
            anthropic_api_key: std::env::var(ANTHROPIC_API_KEY_ENV).ok(),
            openai_api_key: std::env::var(OPENAI_API_KEY_ENV).ok(),
            gemini_api_key: std::env::var(GEMINI_API_KEY_ENV).ok(),
            opencode_api_key: std::env::var(OPENCODE_API_KEY_ENV).ok(),
            extra: HashMap::new(),
        }
    }

    pub fn child_env(self, provider: &AcpProviderSpec) -> Vec<(String, String)> {
        let mut env = Vec::new();
        match provider.launch_kind() {
            AcpLaunchKind::Codex | AcpLaunchKind::Claude => {}
            AcpLaunchKind::Cursor => {
                if let Some(key) = self.anthropic_api_key {
                    env.push((ANTHROPIC_API_KEY_ENV.to_string(), key));
                }
                if let Some(key) = self.openai_api_key {
                    env.push((OPENAI_API_KEY_ENV.to_string(), key));
                }
                if let Some(key) = self.gemini_api_key {
                    env.push((GEMINI_API_KEY_ENV.to_string(), key));
                }
            }
            AcpLaunchKind::OpenCode => {
                if let Some(key) = self.opencode_api_key {
                    env.push((OPENCODE_API_KEY_ENV.to_string(), key));
                }
            }
            AcpLaunchKind::Copilot => {}
        }
        env.extend(self.extra);
        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flavors_only_pipe_required_vendor_keys() {
        let codex = AcpProviderSpec::from_builtin(AcpLaunchKind::Codex);
        let claude = AcpProviderSpec::from_builtin(AcpLaunchKind::Claude);
        let cursor = AcpProviderSpec::from_builtin(AcpLaunchKind::Cursor);
        let opencode = AcpProviderSpec::from_builtin(AcpLaunchKind::OpenCode);
        let copilot = AcpProviderSpec::from_builtin(AcpLaunchKind::Copilot);
        let provider_env = ProviderEnv {
            anthropic_api_key: Some("anthropic".to_string()),
            openai_api_key: Some("openai".to_string()),
            gemini_api_key: Some("gemini".to_string()),
            opencode_api_key: Some("opencode".to_string()),
            extra: HashMap::new(),
        };

        assert!(provider_env.clone().child_env(&codex).is_empty());
        assert!(provider_env.clone().child_env(&claude).is_empty());
        assert_eq!(
            provider_env.clone().child_env(&cursor),
            vec![
                (ANTHROPIC_API_KEY_ENV.to_string(), "anthropic".to_string()),
                (OPENAI_API_KEY_ENV.to_string(), "openai".to_string()),
                (GEMINI_API_KEY_ENV.to_string(), "gemini".to_string()),
            ]
        );
        assert_eq!(
            provider_env.clone().child_env(&opencode),
            vec![(OPENCODE_API_KEY_ENV.to_string(), "opencode".to_string())]
        );
        assert!(provider_env.child_env(&copilot).is_empty());
    }

    #[test]
    fn extra_env_is_preserved_for_explicit_callers() {
        let provider_env = ProviderEnv {
            extra: HashMap::from([("TRACE".to_string(), "1".to_string())]),
            ..ProviderEnv::default()
        };

        assert_eq!(
            provider_env.child_env(&AcpProviderSpec::from_builtin(AcpLaunchKind::Codex)),
            vec![("TRACE".to_string(), "1".to_string())]
        );
    }
}
