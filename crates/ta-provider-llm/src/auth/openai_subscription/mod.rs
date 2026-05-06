use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use ta_auth_openai::browser::{BrowserLaunch, open_authorize_url};
use ta_auth_openai::client::OAuthHttpClient;
use ta_auth_openai::{
    AccessToken, CredentialKey, CredentialStore, OAuthConfig, RefreshPolicy, TokenManager,
    default_chatgpt_subscription_config,
};
use ta_protocol::wire::{AuthProfileId, AuthProfileState};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::error::LlmClientError;
use crate::families::openai::OPENAI_CHATGPT_AUTH_PROFILE_ID;

mod lifecycle;
mod login;
mod logout;
mod profile;

pub const OPENAI_CHATGPT_SUBSCRIPTION_AUTH_PROFILE_ID: &str = OPENAI_CHATGPT_AUTH_PROFILE_ID;
const PENDING_LOGIN_ABORT_TIMEOUT: Duration = Duration::from_millis(250);

type BrowserLauncher = dyn Fn(&Url) -> BrowserLaunch + Send + Sync;

#[derive(Clone)]
pub struct OpenAiSubscriptionAuth {
    inner: Arc<OpenAiSubscriptionAuthInner>,
}

struct OpenAiSubscriptionAuthInner {
    runtime: Handle,
    manager: Arc<TokenManager>,
    store: Arc<dyn CredentialStore>,
    http: Arc<dyn OAuthHttpClient>,
    config: OAuthConfig,
    key: CredentialKey,
    profile_state: Mutex<profile::ProfileRuntimeState>,
    pending_login: Mutex<Option<PendingLoginTask>>,
    lifecycle_spawned: AtomicBool,
    shutdown_token: CancellationToken,
    launch_browser: Arc<BrowserLauncher>,
}

struct PendingLoginTask {
    cancellation: CancellationToken,
    handle: JoinHandle<()>,
}

impl PendingLoginTask {
    async fn cancel(mut self) {
        self.cancellation.cancel();
        self.handle.abort();
        let _ = tokio::time::timeout(PENDING_LOGIN_ABORT_TIMEOUT, &mut self.handle).await;
    }

    #[cfg(test)]
    fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
}

impl Drop for PendingLoginTask {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.handle.abort();
    }
}

impl Drop for OpenAiSubscriptionAuthInner {
    fn drop(&mut self) {
        self.shutdown_token.cancel();
    }
}

impl OpenAiSubscriptionAuth {
    pub fn new(
        runtime: Handle,
        store: Arc<dyn CredentialStore>,
        http: Arc<dyn OAuthHttpClient>,
        config: OAuthConfig,
        policy: RefreshPolicy,
    ) -> Result<Self, LlmClientError> {
        let key = subscription_credential_key()?;
        Ok(Self::with_key(runtime, store, http, config, policy, key))
    }

    pub fn with_key(
        runtime: Handle,
        store: Arc<dyn CredentialStore>,
        http: Arc<dyn OAuthHttpClient>,
        config: OAuthConfig,
        policy: RefreshPolicy,
        key: CredentialKey,
    ) -> Self {
        let manager = Arc::new(TokenManager::new(
            Arc::clone(&store),
            Arc::clone(&http),
            config.clone(),
            policy,
        ));
        Self::from_parts(
            runtime,
            store,
            http,
            config,
            key,
            manager,
            Arc::new(open_authorize_url),
        )
    }

    pub fn from_parts(
        runtime: Handle,
        store: Arc<dyn CredentialStore>,
        http: Arc<dyn OAuthHttpClient>,
        config: OAuthConfig,
        key: CredentialKey,
        manager: Arc<TokenManager>,
        launch_browser: Arc<BrowserLauncher>,
    ) -> Self {
        Self {
            inner: Arc::new(OpenAiSubscriptionAuthInner {
                runtime,
                manager,
                store,
                http,
                config,
                key,
                profile_state: Mutex::new(profile::ProfileRuntimeState::default()),
                pending_login: Mutex::new(None),
                lifecycle_spawned: AtomicBool::new(false),
                shutdown_token: CancellationToken::new(),
                launch_browser,
            }),
        }
    }

    pub fn default_with_store(
        runtime: Handle,
        store: Arc<dyn CredentialStore>,
        http: Arc<dyn OAuthHttpClient>,
    ) -> Result<Self, LlmClientError> {
        let config = default_chatgpt_subscription_config().map_err(map_oauth_error)?;
        Self::new(runtime, store, http, config, RefreshPolicy::default())
    }

    pub fn current_state(&self) -> AuthProfileState {
        self.ensure_lifecycle_listener();
        profile::current_state(self)
    }

    pub async fn bearer(&self) -> Result<String, LlmClientError> {
        self.access_token()
            .await
            .map(|token| token.bearer().to_string())
    }

    pub async fn access_token(&self) -> Result<AccessToken, LlmClientError> {
        self.ensure_lifecycle_listener();
        self.inner
            .manager
            .ensure_fresh(&self.inner.key)
            .await
            .inspect(|token| {
                profile::record_connected(self, token.account().clone());
            })
            .map_err(|error| {
                lifecycle::record_refresh_error(self, &error);
                map_refresh_error(error)
            })
    }

    pub async fn force_refresh(&self) -> Result<String, LlmClientError> {
        self.force_refresh_access_token()
            .await
            .map(|token| token.bearer().to_string())
    }

    pub async fn force_refresh_access_token(&self) -> Result<AccessToken, LlmClientError> {
        self.ensure_lifecycle_listener();
        self.inner
            .manager
            .force_refresh(&self.inner.key)
            .await
            .inspect(|token| {
                profile::record_connected(self, token.account().clone());
            })
            .map_err(|error| {
                lifecycle::record_refresh_error(self, &error);
                map_refresh_error(error)
            })
    }

    pub fn key(&self) -> &CredentialKey {
        &self.inner.key
    }

    pub fn has_platform_api_access_token(&self) -> Result<bool, LlmClientError> {
        self.inner
            .store
            .load(self.key())
            .map(|credentials| {
                credentials.is_some_and(|credentials| {
                    credentials
                        .token_set
                        .api_access_token
                        .as_deref()
                        .is_some_and(|token| !token.trim().is_empty())
                })
            })
            .map_err(map_store_error)
    }

    async fn cancel_pending_login(&self) {
        let pending = self
            .inner
            .pending_login
            .lock()
            .expect("OpenAI subscription pending login state should not be poisoned")
            .take();
        if let Some(task) = pending {
            task.cancel().await;
        }
    }

    async fn replace_pending_login(&self, task: PendingLoginTask) {
        self.cancel_pending_login().await;
        *self
            .inner
            .pending_login
            .lock()
            .expect("OpenAI subscription pending login state should not be poisoned") = Some(task);
    }

    #[cfg(test)]
    fn pending_login_is_finished_for_test(&self) -> Option<bool> {
        self.inner
            .pending_login
            .lock()
            .expect("OpenAI subscription pending login state should not be poisoned")
            .as_ref()
            .map(PendingLoginTask::is_finished)
    }

    fn ensure_lifecycle_listener(&self) {
        if self
            .inner
            .lifecycle_spawned
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let manager = Arc::clone(&self.inner.manager);
        let auth = Arc::downgrade(&self.inner);
        let shutdown_token = self.inner.shutdown_token.clone();
        self.inner.runtime.spawn(async move {
            lifecycle::listen(manager, auth, shutdown_token).await;
        });
    }
}

pub fn subscription_credential_key() -> Result<CredentialKey, LlmClientError> {
    AuthProfileId::new(OPENAI_CHATGPT_SUBSCRIPTION_AUTH_PROFILE_ID)
        .map(CredentialKey::new)
        .map_err(|error| LlmClientError::InvalidConfig(error.to_string()))
}

fn map_refresh_error(error: ta_auth_openai::TokenRefreshError) -> LlmClientError {
    match error {
        ta_auth_openai::TokenRefreshError::NoCredentials => LlmClientError::CredentialsMissing(
            "OpenAI ChatGPT subscription is not signed in".to_string(),
        ),
        ta_auth_openai::TokenRefreshError::AuthRevoked
        | ta_auth_openai::TokenRefreshError::MaxRetriesExceeded { .. }
        | ta_auth_openai::TokenRefreshError::NoRefreshToken
        | ta_auth_openai::TokenRefreshError::TokenExpired => {
            LlmClientError::Auth(error.to_string())
        }
        ta_auth_openai::TokenRefreshError::NetworkError { .. }
        | ta_auth_openai::TokenRefreshError::BackendUnavailable { .. } => {
            LlmClientError::Network(error.to_string())
        }
    }
}

fn profile_lock(
    auth: &OpenAiSubscriptionAuth,
) -> std::sync::MutexGuard<'_, profile::ProfileRuntimeState> {
    auth.inner
        .profile_state
        .lock()
        .expect("OpenAI subscription auth profile state should not be poisoned")
}

fn map_store_error(error: ta_auth_openai::CredentialStoreError) -> LlmClientError {
    LlmClientError::Auth(error.to_string())
}

fn map_oauth_error(error: ta_auth_openai::OAuthError) -> LlmClientError {
    LlmClientError::Auth(error.to_string())
}

#[cfg(test)]
mod tests;
