use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{Mutex, broadcast};

use crate::client::OAuthHttpClient;
use crate::oauth::error_classification::is_revocation_error_code;
use crate::{
    AccountInfo, CredentialKey, CredentialStore, OAuthConfig, OAuthError, StoredCredentials,
    TokenSet,
};

use super::{RefreshPolicy, TokenLifecycleBroadcaster, TokenLifecycleEvent, TokenRefreshError};

pub struct TokenManager {
    store: Arc<dyn CredentialStore>,
    http: Arc<dyn OAuthHttpClient>,
    config: OAuthConfig,
    policy: RefreshPolicy,
    lifecycle: TokenLifecycleBroadcaster,
    refresh_locks: Mutex<HashMap<CredentialKey, Arc<Mutex<()>>>>,
    failure_counts: Mutex<HashMap<CredentialKey, u32>>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken {
    token: String,
    expires_at: SystemTime,
    account: AccountInfo,
    platform_api_token: bool,
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccessToken")
            .field(
                "token",
                &format_args!("<redacted:{} bytes>", self.token.len()),
            )
            .field("expires_at", &self.expires_at)
            .field("account", &self.account)
            .field("platform_api_token", &self.platform_api_token)
            .finish()
    }
}

impl AccessToken {
    pub fn bearer(&self) -> &str {
        &self.token
    }

    pub fn expires_at(&self) -> SystemTime {
        self.expires_at
    }

    pub fn account(&self) -> &AccountInfo {
        &self.account
    }

    pub fn platform_api_token(&self) -> bool {
        self.platform_api_token
    }
}

impl TokenManager {
    pub fn new(
        store: Arc<dyn CredentialStore>,
        http: Arc<dyn OAuthHttpClient>,
        config: OAuthConfig,
        policy: RefreshPolicy,
    ) -> Self {
        Self {
            store,
            http,
            config,
            policy,
            lifecycle: TokenLifecycleBroadcaster::new(),
            refresh_locks: Mutex::new(HashMap::new()),
            failure_counts: Mutex::new(HashMap::new()),
        }
    }

    /// Returns valid access token, refreshing if expired or within proactive_window.
    /// Single-flight: concurrent calls for same key share one refresh.
    pub async fn ensure_fresh(
        &self,
        key: &CredentialKey,
    ) -> Result<AccessToken, TokenRefreshError> {
        let credentials = self.load_credentials(key)?;
        if !self.should_refresh(&credentials) {
            return self.access_token_from_credentials(&credentials);
        }

        let refresh_lock = self.refresh_lock_for(key).await;
        let _guard = refresh_lock.lock().await;
        let credentials = self.load_credentials(key)?;
        if !self.should_refresh(&credentials) {
            return self.access_token_from_credentials(&credentials);
        }
        self.refresh_with_credentials(key, credentials).await
    }

    /// Force refresh (e.g. after 401 from API). Bypasses proactive_window check.
    pub async fn force_refresh(
        &self,
        key: &CredentialKey,
    ) -> Result<AccessToken, TokenRefreshError> {
        let refresh_lock = self.refresh_lock_for(key).await;
        let _guard = refresh_lock.lock().await;
        let credentials = self.load_credentials(key)?;
        self.refresh_with_credentials(key, credentials).await
    }

    /// Logout: revoke refresh token at provider (best-effort) + delete from store + emit event.
    pub async fn logout(&self, key: &CredentialKey) -> Result<(), TokenRefreshError> {
        let Some(credentials) = self
            .store
            .load(key)
            .map_err(|error| TokenRefreshError::from_store(error, "load"))?
        else {
            return Ok(());
        };

        let refresh_token = credentials.token_set.refresh_token.trim();
        if !refresh_token.is_empty()
            && let Err(error) = self
                .http
                .revoke_token(
                    &self.config.revoke_url,
                    &self.config.client_id,
                    refresh_token,
                )
                .await
        {
            tracing::warn!(
                error = %TokenRefreshError::from_oauth(error),
                "failed to revoke OpenAI ChatGPT refresh token during logout; deleting local credentials"
            );
        }

        self.store
            .delete(key)
            .map_err(|error| TokenRefreshError::from_store(error, "delete"))?;
        self.reset_failure_count(key).await;
        self.lifecycle
            .emit(TokenLifecycleEvent::LoggedOut { key: key.clone() });
        Ok(())
    }

    /// Store credentials produced by an interactive login and publish the same
    /// lifecycle signal consumers already use for refreshed credentials.
    pub async fn store_login_credentials(
        &self,
        key: &CredentialKey,
        credentials: StoredCredentials,
    ) -> Result<(), TokenRefreshError> {
        let account = credentials.account.clone();
        self.store
            .store(key, &credentials)
            .map_err(|error| TokenRefreshError::from_store(error, "store"))?;
        self.reset_failure_count(key).await;
        self.lifecycle.emit(TokenLifecycleEvent::Refreshed {
            key: key.clone(),
            account,
        });
        Ok(())
    }

    /// Publish a failed interactive login attempt without exposing provider secrets.
    pub fn emit_login_failed(&self, key: &CredentialKey, reason: String) {
        let reason = crate::oauth::redaction::redact_oauth_error_text(&reason);
        self.lifecycle.emit(TokenLifecycleEvent::LoginFailed {
            key: key.clone(),
            reason,
        });
    }

    /// Subscribe to lifecycle events.
    pub fn subscribe(&self) -> broadcast::Receiver<TokenLifecycleEvent> {
        self.lifecycle.subscribe()
    }

    fn load_credentials(
        &self,
        key: &CredentialKey,
    ) -> Result<StoredCredentials, TokenRefreshError> {
        self.store
            .load(key)
            .map_err(|error| TokenRefreshError::from_store(error, "load"))?
            .ok_or(TokenRefreshError::NoCredentials)
    }

    fn should_refresh(&self, credentials: &StoredCredentials) -> bool {
        let issued_at = issued_at(credentials);
        credentials
            .token_set
            .is_expired_or_within(issued_at, self.policy.proactive_window)
    }

    fn access_token_from_credentials(
        &self,
        credentials: &StoredCredentials,
    ) -> Result<AccessToken, TokenRefreshError> {
        access_token_from_credentials(credentials)
    }

    async fn refresh_lock_for(&self, key: &CredentialKey) -> Arc<Mutex<()>> {
        let mut locks = self.refresh_locks.lock().await;
        locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn refresh_with_credentials(
        &self,
        key: &CredentialKey,
        credentials: StoredCredentials,
    ) -> Result<AccessToken, TokenRefreshError> {
        if credentials.token_set.refresh_token.trim().is_empty() {
            return Err(TokenRefreshError::NoRefreshToken);
        }

        let refresh_result = self
            .http
            .refresh_token(
                &self.config.token_url,
                &self.config.client_id,
                &credentials.token_set.refresh_token,
            )
            .await;

        match refresh_result {
            Ok(mut token_set) => {
                token_set.account_info = credentials.token_set.account_info.clone();
                let updated = refreshed_credentials(credentials, token_set);
                let access_token = access_token_from_credentials(&updated)?;
                self.store
                    .store(key, &updated)
                    .map_err(|error| TokenRefreshError::from_store(error, "store"))?;
                self.reset_failure_count(key).await;
                self.lifecycle.emit(TokenLifecycleEvent::Refreshed {
                    key: key.clone(),
                    account: updated.account,
                });
                Ok(access_token)
            }
            Err(error) if is_auth_revoked(&error) => {
                self.store
                    .delete(key)
                    .map_err(|error| TokenRefreshError::from_store(error, "delete"))?;
                self.reset_failure_count(key).await;
                let reason = TokenRefreshError::AuthRevoked;
                self.lifecycle.emit(TokenLifecycleEvent::NeedsReauth {
                    key: key.clone(),
                    reason: reason.clone(),
                });
                Err(reason)
            }
            Err(error) => {
                let mapped = TokenRefreshError::from_oauth(error);
                let attempts = self.increment_failure_count(key).await;
                self.lifecycle.emit(TokenLifecycleEvent::RefreshFailed {
                    key: key.clone(),
                    error: mapped.clone(),
                });
                if attempts >= self.policy.max_consecutive_failures {
                    let reason = TokenRefreshError::MaxRetriesExceeded { attempts };
                    self.lifecycle.emit(TokenLifecycleEvent::NeedsReauth {
                        key: key.clone(),
                        reason: reason.clone(),
                    });
                    Err(reason)
                } else {
                    Err(mapped)
                }
            }
        }
    }

    async fn increment_failure_count(&self, key: &CredentialKey) -> u32 {
        let mut counts = self.failure_counts.lock().await;
        let count = counts.entry(key.clone()).or_insert(0);
        *count = count.saturating_add(1);
        *count
    }

    async fn reset_failure_count(&self, key: &CredentialKey) {
        self.failure_counts.lock().await.remove(key);
    }
}

fn access_token_from_credentials(
    credentials: &StoredCredentials,
) -> Result<AccessToken, TokenRefreshError> {
    let issued_at = issued_at(credentials);
    let expires_at = credentials
        .token_set
        .expires_at(issued_at)
        .ok_or(TokenRefreshError::TokenExpired)?;
    if expires_at <= SystemTime::now() {
        return Err(TokenRefreshError::TokenExpired);
    }
    let api_access_token = credentials.token_set.api_access_token.clone();
    Ok(AccessToken {
        platform_api_token: api_access_token.is_some(),
        token: api_access_token.unwrap_or_else(|| credentials.token_set.access_token.clone()),
        expires_at,
        account: credentials.account.clone(),
    })
}

fn refreshed_credentials(previous: StoredCredentials, token_set: TokenSet) -> StoredCredentials {
    StoredCredentials {
        token_set,
        account: previous.account,
        stored_at: previous.stored_at,
        last_refreshed_at: Some(now_unix_seconds()),
    }
}

fn issued_at(credentials: &StoredCredentials) -> SystemTime {
    let seconds = credentials
        .last_refreshed_at
        .unwrap_or(credentials.stored_at);
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn is_auth_revoked(error: &OAuthError) -> bool {
    match error {
        OAuthError::TokenEndpointStatus {
            status, error_code, ..
        } => *status == 401 && error_code.as_deref().is_some_and(is_revocation_error_code),
        _ => false,
    }
}
