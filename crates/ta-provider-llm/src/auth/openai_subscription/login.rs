use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::FutureExt;
use ta_auth_openai::browser::BrowserLaunch;
use ta_auth_openai::client::OAuthHttpClient;
use ta_auth_openai::oauth::redaction::redact_oauth_error_text;
use ta_auth_openai::{
    AccountInfo, CompletionHandle, CredentialKey, OpenAiOAuthFlow, StoredCredentials, TokenManager,
    TokenSet,
};
use ta_protocol::wire::{
    AuthProfileId, AuthProfileLoginChallenge, AuthProfileLoginMethod, AuthProfileLoginResult,
};
use tokio_util::sync::CancellationToken;
use url::Url;

use super::{OpenAiSubscriptionAuth, PendingLoginTask, map_oauth_error, profile};
use crate::error::LlmClientError;

impl OpenAiSubscriptionAuth {
    pub async fn login(&self) -> Result<AuthProfileLoginResult, LlmClientError> {
        tracing::info!(
            auth.profile_id = %self.key().as_str(),
            "starting OpenAI ChatGPT OAuth login"
        );
        self.cancel_pending_login().await;
        profile::record_pending_login(self);
        match self.login_inner().await {
            Ok(result) => Ok(result),
            Err(error) => {
                profile::record_login_failed(self, error.to_string());
                Err(error)
            }
        }
    }

    async fn login_inner(&self) -> Result<AuthProfileLoginResult, LlmClientError> {
        let (authorize_url, completion) = OpenAiOAuthFlow::start(self.inner.config.clone())
            .await
            .map_err(map_oauth_error)?;
        tracing::info!(
            auth.profile_id = %self.key().as_str(),
            callback.addr = %completion.callback_addr(),
            "OpenAI ChatGPT OAuth callback server bound"
        );

        match (self.inner.launch_browser)(&authorize_url) {
            BrowserLaunch::Opened => {
                tracing::info!(
                    auth.profile_id = %self.key().as_str(),
                    browser.launch = "auto",
                    "OpenAI ChatGPT OAuth browser launch completed"
                );
                let task = self.spawn_completion_worker(completion);
                self.replace_pending_login(task).await;
                Ok(AuthProfileLoginResult {
                    auth_profile: profile::current_state(self),
                    challenge: Some(browser_challenge(self.key().as_str(), authorize_url)),
                })
            }
            BrowserLaunch::Manual {
                authorize_url,
                reason,
            } => {
                tracing::warn!(
                    auth.profile_id = %self.key().as_str(),
                    reason = %reason,
                    "OpenAI ChatGPT OAuth browser launch failed; returning manual login challenge"
                );
                let task = self.spawn_completion_worker(completion);
                self.replace_pending_login(task).await;
                Ok(AuthProfileLoginResult {
                    auth_profile: profile::current_state(self),
                    challenge: Some(manual_browser_challenge(self.key().as_str(), authorize_url)),
                })
            }
        }
    }

    fn spawn_completion_worker(&self, completion: CompletionHandle) -> PendingLoginTask {
        let inner = Arc::downgrade(&self.inner);
        let http = Arc::clone(&self.inner.http);
        let manager = Arc::clone(&self.inner.manager);
        let key = self.key().clone();
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let handle = self.inner.runtime.spawn(async move {
            let result = AssertUnwindSafe(complete_login(
                completion,
                task_cancellation,
                http,
                manager,
                key,
            ))
            .catch_unwind()
            .await;

            match result {
                Ok(Ok(Some(account))) => {
                    if let Some(inner) = inner.upgrade() {
                        let auth = OpenAiSubscriptionAuth { inner };
                        profile::record_connected(&auth, account);
                    }
                }
                Ok(Ok(None)) => {}
                Ok(Err(error)) => {
                    if let Some(inner) = inner.upgrade() {
                        OpenAiSubscriptionAuth { inner }
                            .record_login_completion_failed(error.to_string());
                    }
                }
                Err(_) => {
                    if let Some(inner) = inner.upgrade() {
                        OpenAiSubscriptionAuth { inner }.record_login_completion_failed(
                            "OpenAI ChatGPT OAuth login completion task panicked".to_string(),
                        );
                    }
                }
            }
        });

        tracing::info!(
            auth.profile_id = %self.key().as_str(),
            "OpenAI ChatGPT OAuth login completion task spawned"
        );
        PendingLoginTask {
            cancellation,
            handle,
        }
    }

    fn record_login_completion_failed(&self, message: String) {
        let reason = redact_oauth_error_text(&message);
        profile::record_login_failed(self, reason.clone());
        self.inner
            .manager
            .emit_login_failed(self.key(), reason.clone());
        tracing::error!(
            auth.profile_id = %self.key().as_str(),
            error = %reason,
            "OpenAI ChatGPT OAuth login completion failed"
        );
    }
}

async fn complete_login(
    completion: CompletionHandle,
    cancellation: CancellationToken,
    http: Arc<dyn OAuthHttpClient>,
    manager: Arc<TokenManager>,
    key: CredentialKey,
) -> Result<Option<AccountInfo>, LlmClientError> {
    let code = tokio::select! {
        () = cancellation.cancelled() => {
            tracing::info!(
                auth.profile_id = %key.as_str(),
                "OpenAI ChatGPT OAuth login completion cancelled"
            );
            return Ok(None);
        }
        code = completion.await_code() => code.map_err(map_oauth_error)?,
    };
    tracing::info!(
        auth.profile_id = %key.as_str(),
        "OpenAI ChatGPT OAuth code received; exchanging tokens"
    );
    let token_set = OpenAiOAuthFlow::exchange_code(http.as_ref(), code)
        .await
        .map_err(map_oauth_error)?;
    if cancellation.is_cancelled() {
        tracing::info!(
            auth.profile_id = %key.as_str(),
            "OpenAI ChatGPT OAuth login completion cancelled before credential persist"
        );
        return Ok(None);
    }
    let account = account_from_token_set(&token_set);
    let credentials = StoredCredentials {
        token_set,
        account: account.clone(),
        stored_at: now_unix_seconds(),
        last_refreshed_at: None,
    };
    manager
        .store_login_credentials(&key, credentials)
        .await
        .map_err(super::map_refresh_error)?;
    tracing::info!(
        auth.profile_id = %key.as_str(),
        "OpenAI ChatGPT OAuth tokens persisted"
    );
    Ok(Some(account))
}

fn browser_challenge(profile_id: &str, authorize_url: Url) -> AuthProfileLoginChallenge {
    AuthProfileLoginChallenge {
        auth_profile_id: AuthProfileId::new(profile_id)
            .expect("credential key is sourced from a valid auth profile id"),
        method: AuthProfileLoginMethod::Browser,
        manual_browser_url: None,
        authorize_url: Some(authorize_url.to_string()),
        user_code: None,
    }
}

fn manual_browser_challenge(profile_id: &str, authorize_url: Url) -> AuthProfileLoginChallenge {
    let authorize_url = authorize_url.to_string();
    AuthProfileLoginChallenge {
        auth_profile_id: AuthProfileId::new(profile_id)
            .expect("credential key is sourced from a valid auth profile id"),
        method: AuthProfileLoginMethod::Manual,
        manual_browser_url: Some(authorize_url.clone()),
        authorize_url: Some(authorize_url),
        user_code: None,
    }
}

fn account_from_token_set(token_set: &TokenSet) -> AccountInfo {
    let info = token_set.account_info.as_ref();
    AccountInfo {
        account_id: info
            .and_then(|info| info.account_id.clone())
            .unwrap_or_default(),
        email: info.and_then(|info| info.email.clone()).unwrap_or_default(),
        organization_id: info.and_then(|info| info.organization_id.clone()),
        plan_tier: info.and_then(|info| info.plan_type.clone()),
    }
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
