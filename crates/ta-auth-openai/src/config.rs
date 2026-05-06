use std::time::Duration;

use url::Url;

use crate::error::OAuthError;
pub use crate::oauth::endpoints::{
    OPENAI_CHATGPT_AUTH_URL, OPENAI_CHATGPT_CALLBACK_PORTS, OPENAI_CHATGPT_CLIENT_ID,
    OPENAI_CHATGPT_ORIGINATOR, OPENAI_CHATGPT_REDIRECT_URI_TEMPLATE, OPENAI_CHATGPT_REVOKE_URL,
    OPENAI_CHATGPT_SCOPES, OPENAI_CHATGPT_TOKEN_URL,
};

#[derive(Clone, Debug)]
pub struct OAuthConfig {
    pub auth_url: Url,
    pub token_url: Url,
    pub revoke_url: Url,
    pub client_id: String,
    pub scopes: Vec<String>,
    pub redirect_uri_template: String,
    pub callback_ports: Vec<u16>,
    pub callback_timeout: Duration,
    pub originator: Option<String>,
    pub allowed_workspace_id: Option<String>,
}

impl OAuthConfig {
    pub fn build_redirect_uri(&self, port: u16) -> Result<String, OAuthError> {
        if !self.redirect_uri_template.contains("{port}") {
            return Err(OAuthError::InvalidConfig(
                "redirect_uri_template must contain `{port}`".to_string(),
            ));
        }
        Ok(self
            .redirect_uri_template
            .replace("{port}", &port.to_string()))
    }

    pub fn scope_value(&self) -> String {
        self.scopes.join(" ")
    }

    pub(crate) fn authorize_originator(&self) -> Option<&str> {
        // The shared ChatGPT OAuth client id is valid only with Codex's originator.
        if self.client_id == OPENAI_CHATGPT_CLIENT_ID {
            return Some(OPENAI_CHATGPT_ORIGINATOR);
        }
        self.originator.as_deref()
    }
}

pub fn default_chatgpt_subscription_config() -> Result<OAuthConfig, OAuthError> {
    Ok(OAuthConfig {
        auth_url: Url::parse(OPENAI_CHATGPT_AUTH_URL).map_err(|source| OAuthError::InvalidUrl {
            field: "auth_url",
            source,
        })?,
        token_url: Url::parse(OPENAI_CHATGPT_TOKEN_URL).map_err(|source| {
            OAuthError::InvalidUrl {
                field: "token_url",
                source,
            }
        })?,
        revoke_url: Url::parse(OPENAI_CHATGPT_REVOKE_URL).map_err(|source| {
            OAuthError::InvalidUrl {
                field: "revoke_url",
                source,
            }
        })?,
        client_id: OPENAI_CHATGPT_CLIENT_ID.to_string(),
        scopes: OPENAI_CHATGPT_SCOPES
            .iter()
            .map(|scope| (*scope).to_string())
            .collect(),
        redirect_uri_template: OPENAI_CHATGPT_REDIRECT_URI_TEMPLATE.to_string(),
        callback_ports: OPENAI_CHATGPT_CALLBACK_PORTS.to_vec(),
        callback_timeout: Duration::from_secs(5 * 60),
        originator: Some(OPENAI_CHATGPT_ORIGINATOR.to_string()),
        allowed_workspace_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{OPENAI_CHATGPT_ORIGINATOR, default_chatgpt_subscription_config};

    #[test]
    fn default_config_uses_discovered_endpoint_values() -> Result<(), Box<dyn std::error::Error>> {
        let config = default_chatgpt_subscription_config()?;

        assert_eq!(
            config.auth_url.as_str(),
            "https://auth.openai.com/oauth/authorize"
        );
        assert_eq!(
            config.token_url.as_str(),
            "https://auth.openai.com/oauth/token"
        );
        assert_eq!(
            config.revoke_url.as_str(),
            "https://auth.openai.com/oauth/revoke"
        );
        assert_eq!(config.client_id, "app_EMoamEEZ73f0CkXaXp7hrann");
        assert_eq!(config.originator.as_deref(), Some("codex_cli_rs"));
        assert_eq!(
            config.scopes.join(" "),
            "openid profile email offline_access api.connectors.read api.connectors.invoke"
        );
        assert_eq!(
            config.build_redirect_uri(1455)?,
            "http://localhost:1455/auth/callback"
        );
        Ok(())
    }

    #[test]
    fn chatgpt_client_id_always_authorizes_with_codex_originator()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut config = default_chatgpt_subscription_config()?;
        config.originator = Some("taugentic".to_string());

        assert_eq!(
            config.authorize_originator(),
            Some(OPENAI_CHATGPT_ORIGINATOR)
        );

        config.client_id = "custom-client".to_string();
        assert_eq!(config.authorize_originator(), Some("taugentic"));
        Ok(())
    }
}
