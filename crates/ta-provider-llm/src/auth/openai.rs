use std::env;
use std::sync::{Arc, OnceLock};
use ta_auth_openai::client::ReqwestOAuthHttpClient;
use ta_protocol::wire::{
    AgentRuntimeStrategyId, AuthProfileConnectionState, AuthProfileId, AuthProfileLoginResult,
    AuthProfileLogoutResult, AuthProfileManagementMode, AuthProfileMethodInfo, AuthProfileRef,
    AuthProfileState,
};
use tokio::runtime::Handle;

use super::openai_subscription::OpenAiSubscriptionAuth;
use crate::error::LlmClientError;
use crate::families::openai::{
    OPENAI_API_KEY_AUTH_PROFILE_ID, OPENAI_API_KEY_ENV_VAR, OPENAI_CHATGPT_AUTH_PROFILE_ID,
    OPENAI_PROVIDER_ID,
};
use crate::http::shared_client;

static DEFAULT_SUBSCRIPTION_AUTH: OnceLock<OpenAiSubscriptionAuth> = OnceLock::new();

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiAuthSnapshot {
    pub api_key_configured: bool,
    pub chatgpt_configured: bool,
    pub auth_profiles: Vec<AuthProfileState>,
}

pub fn auth_profile_refs() -> Vec<AuthProfileRef> {
    vec![
        auth_profile_ref(OPENAI_API_KEY_AUTH_PROFILE_ID, "OpenAI API Key"),
        auth_profile_ref(
            OPENAI_CHATGPT_AUTH_PROFILE_ID,
            "OpenAI ChatGPT Subscription",
        ),
    ]
}

pub fn snapshot() -> OpenAiAuthSnapshot {
    let api_key_configured = env_secret_configured(OPENAI_API_KEY_ENV_VAR);
    let subscription_state = default_subscription_auth()
        .map(|auth| auth.current_state())
        .unwrap_or_else(subscription_unavailable_state);
    let chatgpt_configured =
        subscription_state.connection_state == AuthProfileConnectionState::Connected;
    snapshot_for_connection(api_key_configured, subscription_state, chatgpt_configured)
}

pub fn auth_for_profile(
    auth_profile_id: Option<&AuthProfileId>,
) -> Result<OpenAiAuth, LlmClientError> {
    match auth_profile_id.map(AuthProfileId::as_str) {
        None | Some(OPENAI_API_KEY_AUTH_PROFILE_ID) => {
            api_key_auth_from_env_value(std::env::var(OPENAI_API_KEY_ENV_VAR))
        }
        Some(OPENAI_CHATGPT_AUTH_PROFILE_ID) => Ok(OpenAiAuth::Subscription {
            auth: default_subscription_auth()?,
        }),
        Some(other) => Err(unknown_auth_profile(other)),
    }
}

pub async fn login(
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLoginResult, LlmClientError> {
    match auth_profile_id.as_str() {
        OPENAI_API_KEY_AUTH_PROFILE_ID => {
            if !env_secret_configured(OPENAI_API_KEY_ENV_VAR) {
                return Err(LlmClientError::CredentialsMissing(format!(
                    "{OPENAI_API_KEY_ENV_VAR} is not set"
                )));
            }
            Ok(AuthProfileLoginResult {
                auth_profile: auth_profile_state(
                    OPENAI_API_KEY_AUTH_PROFILE_ID,
                    "OpenAI API Key",
                    AuthProfileConnectionState::Connected,
                    None,
                ),
                challenge: None,
            })
        }
        OPENAI_CHATGPT_AUTH_PROFILE_ID => {
            let auth = default_subscription_auth()?;
            auth.login().await
        }
        other => Err(unknown_auth_profile(other)),
    }
}

pub async fn logout(
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLogoutResult, LlmClientError> {
    match auth_profile_id.as_str() {
        OPENAI_API_KEY_AUTH_PROFILE_ID => Ok(AuthProfileLogoutResult {
            auth_profile_id: auth_profile_id.clone(),
            disconnected: false,
        }),
        OPENAI_CHATGPT_AUTH_PROFILE_ID => {
            let auth = default_subscription_auth()?;
            auth.logout().await
        }
        other => Err(unknown_auth_profile(other)),
    }
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

pub fn initialize_default_subscription_auth(
    runtime: Handle,
) -> Result<OpenAiSubscriptionAuth, LlmClientError> {
    if let Some(auth) = DEFAULT_SUBSCRIPTION_AUTH.get() {
        return Ok(auth.clone());
    }
    let store =
        ta_auth_openai::default_store().map_err(|error| LlmClientError::Auth(error.to_string()))?;
    let http: Arc<dyn ta_auth_openai::client::OAuthHttpClient> =
        Arc::new(ReqwestOAuthHttpClient::new(shared_client()));
    let auth = OpenAiSubscriptionAuth::default_with_store(runtime, store, http)?;
    let _ = DEFAULT_SUBSCRIPTION_AUTH.set(auth);
    DEFAULT_SUBSCRIPTION_AUTH.get().cloned().ok_or_else(|| {
        LlmClientError::Auth("OpenAI subscription auth did not initialize".to_string())
    })
}

fn default_subscription_auth() -> Result<OpenAiSubscriptionAuth, LlmClientError> {
    if let Some(auth) = DEFAULT_SUBSCRIPTION_AUTH.get() {
        return Ok(auth.clone());
    }
    let runtime = Handle::try_current().map_err(|error| {
        LlmClientError::ProcessFailed(format!(
            "OpenAI subscription auth requires a Tokio runtime: {error}"
        ))
    })?;
    initialize_default_subscription_auth(runtime)
}

fn snapshot_for_connection(
    api_key_configured: bool,
    subscription_state: AuthProfileState,
    chatgpt_configured: bool,
) -> OpenAiAuthSnapshot {
    OpenAiAuthSnapshot {
        api_key_configured,
        chatgpt_configured,
        auth_profiles: vec![
            auth_profile_state(
                OPENAI_API_KEY_AUTH_PROFILE_ID,
                "OpenAI API Key",
                if api_key_configured {
                    AuthProfileConnectionState::Connected
                } else {
                    AuthProfileConnectionState::LoggedOut
                },
                None,
            ),
            subscription_state,
        ],
    }
}

fn auth_profile_ref(auth_profile_id: &str, display_name: &str) -> AuthProfileRef {
    AuthProfileRef {
        id: AuthProfileId::new(auth_profile_id).expect("auth profile id"),
        provider_id: AgentRuntimeStrategyId::new(OPENAI_PROVIDER_ID).expect("provider id"),
        display_name: display_name.to_string(),
    }
}

fn auth_profile_state(
    auth_profile_id: &str,
    display_name: &str,
    connection_state: AuthProfileConnectionState,
    last_error: Option<String>,
) -> AuthProfileState {
    let management_mode = if auth_profile_id == OPENAI_API_KEY_AUTH_PROFILE_ID {
        AuthProfileManagementMode::Environment
    } else {
        AuthProfileManagementMode::Interactive
    };

    AuthProfileState {
        profile: auth_profile_ref(auth_profile_id, display_name),
        connection_state,
        last_error,
        management_mode: management_mode.clone(),
        can_login: auth_profile_id == OPENAI_API_KEY_AUTH_PROFILE_ID
            && connection_state != AuthProfileConnectionState::Connected,
        can_logout: false,
        platform_org_linked: None,
        setup_steps: if auth_profile_id == OPENAI_API_KEY_AUTH_PROFILE_ID {
            vec![format!(
                "Set {OPENAI_API_KEY_ENV_VAR} in the daemon environment"
            )]
        } else {
            Vec::new()
        },
        action: None,
        methods: vec![AuthProfileMethodInfo {
            id: auth_profile_id.to_string(),
            display_name: display_name.to_string(),
            management_mode,
        }],
    }
}

fn subscription_unavailable_state(error: LlmClientError) -> AuthProfileState {
    let mut state = auth_profile_state(
        OPENAI_CHATGPT_AUTH_PROFILE_ID,
        "OpenAI ChatGPT Subscription",
        AuthProfileConnectionState::Error,
        Some(error.to_string()),
    );
    state.can_login = false;
    state.can_logout = false;
    state
}

fn env_secret_configured(env_var: &str) -> bool {
    env::var(env_var).is_ok_and(|value| !value.trim().is_empty())
}

fn unknown_auth_profile(auth_profile_id: &str) -> LlmClientError {
    LlmClientError::InvalidConfig(format!("unknown OpenAI auth profile {auth_profile_id}"))
}

fn api_key_auth_from_env_value(
    key: Result<String, env::VarError>,
) -> Result<OpenAiAuth, LlmClientError> {
    let key = key.map_err(|_| {
        LlmClientError::CredentialsMissing(openai_api_key_profile_error("is not set"))
    })?;
    if key.trim().is_empty() {
        return Err(LlmClientError::CredentialsMissing(
            openai_api_key_profile_error("is empty"),
        ));
    }
    Ok(OpenAiAuth::ApiKey { key })
}

pub(crate) fn openai_api_key_profile_error(state: &str) -> String {
    format!(
        "{OPENAI_API_KEY_ENV_VAR} {state}. This runtime profile uses an OpenAI Platform API key. To use a ChatGPT subscription, select an OpenAI ChatGPT Allow/Safe/Deny runtime profile instead."
    )
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
            "OpenAI ChatGPT account id is missing from subscription credentials; sign in again"
                .to_string(),
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

    fn auth_profile_id(value: &str) -> AuthProfileId {
        AuthProfileId::new(value).expect("auth profile id")
    }

    #[test]
    fn exposes_api_key_and_chatgpt_subscription_profiles() {
        let refs = auth_profile_refs();

        assert_eq!(refs.len(), 2);
        assert!(
            refs.iter()
                .any(|profile| profile.id.as_str() == OPENAI_API_KEY_AUTH_PROFILE_ID)
        );
        assert!(
            refs.iter()
                .any(|profile| profile.id.as_str() == OPENAI_CHATGPT_AUTH_PROFILE_ID)
        );
        assert!(
            refs.iter()
                .all(|profile| profile.provider_id.as_str() == OPENAI_PROVIDER_ID)
        );
    }

    #[test]
    fn snapshot_keeps_subscription_separate_from_api_key_state() {
        let subscription_state = auth_profile_state(
            OPENAI_CHATGPT_AUTH_PROFILE_ID,
            "OpenAI ChatGPT Subscription",
            AuthProfileConnectionState::LoggedOut,
            None,
        );
        let snapshot = snapshot_for_connection(true, subscription_state, false);

        let api_key = snapshot
            .auth_profiles
            .iter()
            .find(|profile| profile.profile.id.as_str() == OPENAI_API_KEY_AUTH_PROFILE_ID)
            .expect("api key profile");
        let chatgpt = snapshot
            .auth_profiles
            .iter()
            .find(|profile| profile.profile.id.as_str() == OPENAI_CHATGPT_AUTH_PROFILE_ID)
            .expect("chatgpt profile");

        assert_eq!(
            api_key.connection_state,
            AuthProfileConnectionState::Connected
        );
        assert_eq!(
            chatgpt.connection_state,
            AuthProfileConnectionState::LoggedOut
        );
        assert!(snapshot.api_key_configured);
        assert!(!snapshot.chatgpt_configured);
    }

    #[tokio::test]
    async fn chatgpt_subscription_credential_resolution_uses_native_auth() {
        let auth = auth_for_profile(Some(&auth_profile_id(OPENAI_CHATGPT_AUTH_PROFILE_ID)))
            .expect("native subscription auth should resolve");

        assert!(matches!(auth, OpenAiAuth::Subscription { .. }));
    }

    #[test]
    fn api_key_profile_missing_error_points_to_chatgpt_runtime_profiles() {
        assert!(matches!(
            api_key_auth_from_env_value(Err(env::VarError::NotPresent)),
            Err(LlmClientError::CredentialsMissing(message))
                if message == "OPENAI_API_KEY is not set. This runtime profile uses an OpenAI Platform API key. To use a ChatGPT subscription, select an OpenAI ChatGPT Allow/Safe/Deny runtime profile instead."
        ));
        assert!(matches!(
            api_key_auth_from_env_value(Ok("   ".to_string())),
            Err(LlmClientError::CredentialsMissing(message))
                if message == "OPENAI_API_KEY is empty. This runtime profile uses an OpenAI Platform API key. To use a ChatGPT subscription, select an OpenAI ChatGPT Allow/Safe/Deny runtime profile instead."
        ));
    }
}
