use std::sync::Arc;

use ta_auth_openai::client::ReqwestOAuthHttpClient;
use ta_protocol::wire::{
    AuthMethodId, AuthProfileId, AuthProfileLoginResult, AuthProfileLogoutResult,
};
use tokio::runtime::Handle;

use super::openai_subscription::OpenAiSubscriptionAuth;
use crate::error::LlmClientError;
use crate::http::shared_client;

const OPENAI_CHATGPT_AUTH_METHOD_ID: &str = "openai-chatgpt";
pub(crate) const OPENAI_PLATFORM_RESPONSES_BASE_URL: &str = "https://api.openai.com/v1";
pub(crate) const OPENAI_CHATGPT_RESPONSES_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpenAiAuthRoute {
    base_url: String,
    bearer_token: String,
    organization_id: Option<String>,
    chatgpt_account_id: Option<String>,
    label_for_logs: &'static str,
}

impl OpenAiAuthRoute {
    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }
    pub(crate) fn bearer_token(&self) -> &str {
        &self.bearer_token
    }
    pub(crate) fn organization_id(&self) -> Option<&str> {
        self.organization_id.as_deref()
    }
    pub(crate) fn chatgpt_account_id(&self) -> Option<&str> {
        self.chatgpt_account_id.as_deref()
    }
    pub(crate) fn label_for_logs(&self) -> &'static str {
        self.label_for_logs
    }
    pub(crate) fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

#[derive(Clone)]
pub enum OpenAiAuth {
    ApiKey { key: String },
    Subscription { auth: OpenAiSubscriptionAuth },
}

pub fn auth_for_profile(
    auth_profile_id: Option<&AuthProfileId>,
) -> Result<OpenAiAuth, LlmClientError> {
    let auth_profile_id = auth_profile_id.ok_or_else(|| {
        LlmClientError::CredentialsMissing(
            "OpenAI execution requires an explicit connected auth profile".to_string(),
        )
    })?;
    Ok(OpenAiAuth::Subscription {
        auth: subscription_auth(auth_profile_id)?,
    })
}

pub async fn login(
    auth_method_id: &AuthMethodId,
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLoginResult, LlmClientError> {
    validate_method(auth_method_id)?;
    subscription_auth(auth_profile_id)?.login().await
}

pub async fn logout(
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLogoutResult, LlmClientError> {
    subscription_auth(auth_profile_id)?.logout().await
}

impl OpenAiAuth {
    pub(crate) async fn route(&self) -> Result<OpenAiAuthRoute, LlmClientError> {
        match self {
            OpenAiAuth::ApiKey { key } => Ok(OpenAiAuthRoute {
                base_url: OPENAI_PLATFORM_RESPONSES_BASE_URL.to_string(),
                bearer_token: key.clone(),
                organization_id: None,
                chatgpt_account_id: None,
                label_for_logs: "openai_platform_api_key",
            }),
            OpenAiAuth::Subscription { auth } => {
                route_from_subscription_token(auth.access_token().await?)
            }
        }
    }

    pub(crate) async fn force_refresh_route(
        &self,
    ) -> Result<Option<OpenAiAuthRoute>, LlmClientError> {
        match self {
            OpenAiAuth::ApiKey { .. } => Ok(None),
            OpenAiAuth::Subscription { auth } => {
                route_from_subscription_token(auth.force_refresh_access_token().await?).map(Some)
            }
        }
    }
}

pub(crate) fn openai_api_key_profile_error(state: &str) -> String {
    format!("OPENAI_API_KEY {state}")
}

fn subscription_auth(
    auth_profile_id: &AuthProfileId,
) -> Result<OpenAiSubscriptionAuth, LlmClientError> {
    let runtime = Handle::try_current().map_err(|error| {
        LlmClientError::ProcessFailed(format!(
            "OpenAI subscription auth requires a Tokio runtime: {error}"
        ))
    })?;
    let store =
        ta_auth_openai::default_store().map_err(|error| LlmClientError::Auth(error.to_string()))?;
    let http: Arc<dyn ta_auth_openai::client::OAuthHttpClient> =
        Arc::new(ReqwestOAuthHttpClient::new(shared_client()));
    OpenAiSubscriptionAuth::default_with_store(runtime, store, http, auth_profile_id.clone())
}

fn validate_method(auth_method_id: &AuthMethodId) -> Result<(), LlmClientError> {
    if auth_method_id.as_str() == OPENAI_CHATGPT_AUTH_METHOD_ID {
        Ok(())
    } else {
        Err(LlmClientError::InvalidConfig(
            "OpenAI auth method is not supported".to_string(),
        ))
    }
}

fn route_from_subscription_token(
    token: ta_auth_openai::AccessToken,
) -> Result<OpenAiAuthRoute, LlmClientError> {
    if token.platform_api_token() {
        return Ok(OpenAiAuthRoute {
            base_url: OPENAI_PLATFORM_RESPONSES_BASE_URL.to_string(),
            bearer_token: token.bearer().to_string(),
            organization_id: token.account().organization_id.clone(),
            chatgpt_account_id: None,
            label_for_logs: "openai_platform_subscription",
        });
    }
    let account_id = token.account().account_id.trim();
    if account_id.is_empty() {
        return Err(LlmClientError::Auth(
            "OpenAI ChatGPT account identity is missing; sign in again".to_string(),
        ));
    }
    Ok(OpenAiAuthRoute {
        base_url: OPENAI_CHATGPT_RESPONSES_BASE_URL.to_string(),
        bearer_token: token.bearer().to_string(),
        organization_id: None,
        chatgpt_account_id: Some(account_id.to_string()),
        label_for_logs: "openai_chatgpt_subscription",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_unowned_auth_method() {
        let method = AuthMethodId::new("different-provider").expect("auth method");
        assert!(matches!(
            validate_method(&method),
            Err(LlmClientError::InvalidConfig(_))
        ));
    }
}
