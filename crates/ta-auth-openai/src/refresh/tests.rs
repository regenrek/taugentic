use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ta_protocol::wire::AuthProfileId;
use tokio::sync::Barrier;
use url::Url;

use crate::client::{OAuthHttpClient, OAuthHttpFuture, OAuthTokenFuture, OAuthUnitFuture};
use crate::{
    AccountInfo, CredentialKey, CredentialStore, CredentialStoreError, OAuthConfig, OAuthError,
    StoredCredentials, TokenSet,
};

use super::{RefreshPolicy, TokenLifecycleEvent, TokenManager, TokenRefreshError};

#[tokio::test]
async fn ensure_fresh_with_valid_token_returns_existing() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("existing-access", "refresh", now(), 3600),
    )?;
    let http = Arc::new(MockOAuthHttpClient::default());
    let manager = manager(store, http.clone())?;

    let token = manager.ensure_fresh(&key).await?;

    assert_eq!(token.bearer(), "existing-access");
    assert!(!token.platform_api_token());
    assert_eq!(http.refresh_call_count(), 0);
    Ok(())
}

#[tokio::test]
async fn ensure_fresh_prefers_api_access_token_for_bearer() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    let mut credentials = stored_credentials("oauth-access", "refresh", now(), 3600);
    credentials.token_set.api_access_token = Some("api-access".to_string());
    store.store(&key, &credentials)?;
    let manager = manager(store, Arc::new(MockOAuthHttpClient::default()))?;

    let token = manager.ensure_fresh(&key).await?;

    assert_eq!(token.bearer(), "api-access");
    assert!(token.platform_api_token());
    Ok(())
}

#[tokio::test]
async fn ensure_fresh_with_token_in_proactive_window_refreshes_and_emits()
-> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("old-access", "refresh", now(), 60),
    )?;
    let http = Arc::new(MockOAuthHttpClient::with_refresh(vec![Ok(token_set(
        "fresh-access",
        "fresh-refresh",
        3600,
    ))]));
    let manager = manager(store.clone(), http.clone())?;
    let mut events = manager.subscribe();

    let token = manager.ensure_fresh(&key).await?;

    assert_eq!(token.bearer(), "fresh-access");
    assert_eq!(http.refresh_call_count(), 1);
    assert_eq!(
        store
            .load(&key)?
            .expect("credentials")
            .token_set
            .access_token,
        "fresh-access"
    );
    assert_eq!(
        events.recv().await?,
        TokenLifecycleEvent::Refreshed {
            key,
            account: account()
        }
    );
    Ok(())
}

#[tokio::test]
async fn ensure_fresh_with_expired_token_refreshes() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("expired-access", "refresh", now() - 7200, 60),
    )?;
    let http = Arc::new(MockOAuthHttpClient::with_refresh(vec![Ok(token_set(
        "fresh-access",
        "fresh-refresh",
        3600,
    ))]));
    let manager = manager(store, http.clone())?;

    let token = manager.ensure_fresh(&key).await?;

    assert_eq!(token.bearer(), "fresh-access");
    assert_eq!(http.refresh_call_count(), 1);
    Ok(())
}

#[tokio::test]
async fn ensure_fresh_single_flights_concurrent_refreshes() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("expired-access", "refresh", now() - 7200, 60),
    )?;
    let http = Arc::new(MockOAuthHttpClient::with_refresh(vec![Ok(token_set(
        "fresh-access",
        "fresh-refresh",
        3600,
    ))]));
    let manager = Arc::new(manager(store, http.clone())?);
    let barrier = Arc::new(Barrier::new(10));
    let mut tasks = Vec::new();

    for _ in 0..10 {
        let manager = Arc::clone(&manager);
        let key = key.clone();
        let barrier = Arc::clone(&barrier);
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            manager.ensure_fresh(&key).await
        }));
    }

    for task in tasks {
        assert_eq!(task.await??.bearer(), "fresh-access");
    }
    assert_eq!(http.refresh_call_count(), 1);
    Ok(())
}

#[tokio::test]
async fn force_refresh_bypasses_window_check() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("existing-access", "refresh", now(), 3600),
    )?;
    let http = Arc::new(MockOAuthHttpClient::with_refresh(vec![Ok(token_set(
        "forced-access",
        "forced-refresh",
        3600,
    ))]));
    let manager = manager(store, http.clone())?;

    let token = manager.force_refresh(&key).await?;

    assert_eq!(token.bearer(), "forced-access");
    assert_eq!(http.refresh_call_count(), 1);
    Ok(())
}

#[tokio::test]
async fn invalid_grant_deletes_credentials_and_emits_needs_reauth() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("expired-access", "refresh", now() - 7200, 60),
    )?;
    let http = Arc::new(MockOAuthHttpClient::with_refresh(vec![Err(
        OAuthError::TokenEndpointStatus {
            status: 401,
            error_code: Some("invalid_grant".to_string()),
            message: "refresh failed".to_string(),
        },
    )]));
    let manager = manager(store.clone(), http)?;
    let mut events = manager.subscribe();

    let error = manager
        .ensure_fresh(&key)
        .await
        .expect_err("refresh should fail");

    assert_eq!(error, TokenRefreshError::AuthRevoked);
    assert_eq!(store.load(&key)?, None);
    assert_eq!(
        events.recv().await?,
        TokenLifecycleEvent::NeedsReauth {
            key,
            reason: TokenRefreshError::AuthRevoked
        }
    );
    Ok(())
}

#[tokio::test]
async fn invalid_token_deletes_credentials_and_emits_needs_reauth() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("expired-access", "refresh", now() - 7200, 60),
    )?;
    let http = Arc::new(MockOAuthHttpClient::with_refresh(vec![Err(
        OAuthError::TokenEndpointStatus {
            status: 401,
            error_code: Some("invalid_token".to_string()),
            message: "refresh token revoked".to_string(),
        },
    )]));
    let manager = manager(store.clone(), http)?;

    let error = manager
        .force_refresh(&key)
        .await
        .expect_err("refresh should fail");

    assert_eq!(error, TokenRefreshError::AuthRevoked);
    assert_eq!(store.load(&key)?, None);
    Ok(())
}

#[tokio::test]
async fn server_error_401_preserves_credentials_and_counts_failure() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("expired-access", "refresh", now() - 7200, 60),
    )?;
    let http = Arc::new(MockOAuthHttpClient::with_refresh(vec![Err(
        OAuthError::TokenEndpointStatus {
            status: 401,
            error_code: Some("server_error".to_string()),
            message: "temporary auth proxy failure".to_string(),
        },
    )]));
    let manager = manager(store.clone(), http)?;
    let mut events = manager.subscribe();

    let error = manager
        .force_refresh(&key)
        .await
        .expect_err("refresh should fail");

    assert_eq!(error, TokenRefreshError::NetworkError { status: Some(401) });
    assert!(store.load(&key)?.is_some());
    assert!(matches!(
        events.recv().await?,
        TokenLifecycleEvent::RefreshFailed { .. }
    ));
    Ok(())
}

#[tokio::test]
async fn unclassified_401_preserves_credentials() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("expired-access", "refresh", now() - 7200, 60),
    )?;
    let http = Arc::new(MockOAuthHttpClient::with_refresh(vec![
        Err(OAuthError::TokenEndpointStatus {
            status: 401,
            error_code: None,
            message: "proxy login page".to_string(),
        }),
        Err(OAuthError::TokenEndpointStatus {
            status: 401,
            error_code: None,
            message: "empty response body".to_string(),
        }),
    ]));
    let manager = manager_with_policy(
        store.clone(),
        http,
        RefreshPolicy {
            max_consecutive_failures: 3,
            ..RefreshPolicy::default()
        },
    )?;

    assert_eq!(
        manager.force_refresh(&key).await,
        Err(TokenRefreshError::NetworkError { status: Some(401) })
    );
    assert_eq!(
        manager.force_refresh(&key).await,
        Err(TokenRefreshError::NetworkError { status: Some(401) })
    );
    assert!(store.load(&key)?.is_some());
    Ok(())
}

#[tokio::test]
async fn network_errors_emit_needs_reauth_after_max_failures() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("expired-access", "refresh", now() - 7200, 60),
    )?;
    let http = Arc::new(MockOAuthHttpClient::with_refresh(vec![
        Err(OAuthError::HttpTransport(
            "timeout token=secret".to_string(),
        )),
        Err(OAuthError::HttpTransport(
            "timeout token=secret".to_string(),
        )),
    ]));
    let manager = manager_with_policy(
        store.clone(),
        http,
        RefreshPolicy {
            max_consecutive_failures: 2,
            ..RefreshPolicy::default()
        },
    )?;
    let mut events = manager.subscribe();

    assert!(matches!(
        manager.force_refresh(&key).await,
        Err(TokenRefreshError::NetworkError { status: None })
    ));
    assert_eq!(
        manager.force_refresh(&key).await,
        Err(TokenRefreshError::MaxRetriesExceeded { attempts: 2 })
    );
    assert!(store.load(&key)?.is_some());
    assert!(matches!(
        events.recv().await?,
        TokenLifecycleEvent::RefreshFailed { .. }
    ));
    assert!(matches!(
        events.recv().await?,
        TokenLifecycleEvent::RefreshFailed { .. }
    ));
    assert_eq!(
        events.recv().await?,
        TokenLifecycleEvent::NeedsReauth {
            key,
            reason: TokenRefreshError::MaxRetriesExceeded { attempts: 2 }
        }
    );
    Ok(())
}

#[tokio::test]
async fn successful_refresh_resets_failure_counter() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("expired-access", "refresh", now() - 7200, 60),
    )?;
    let http = Arc::new(MockOAuthHttpClient::with_refresh(vec![
        Err(OAuthError::HttpTransport("temporary".to_string())),
        Ok(token_set("fresh-access", "fresh-refresh", 3600)),
        Err(OAuthError::HttpTransport("temporary".to_string())),
    ]));
    let manager = manager_with_policy(
        store,
        http,
        RefreshPolicy {
            max_consecutive_failures: 2,
            ..RefreshPolicy::default()
        },
    )?;

    assert!(matches!(
        manager.force_refresh(&key).await,
        Err(TokenRefreshError::NetworkError { status: None })
    ));
    assert_eq!(manager.force_refresh(&key).await?.bearer(), "fresh-access");
    assert!(matches!(
        manager.force_refresh(&key).await,
        Err(TokenRefreshError::NetworkError { status: None })
    ));
    Ok(())
}

#[tokio::test]
async fn logout_deletes_credentials_and_emits_logged_out() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(&key, &stored_credentials("access", "refresh", now(), 3600))?;
    let http = Arc::new(MockOAuthHttpClient::with_revoke(vec![Ok(())]));
    let manager = manager(store.clone(), http.clone())?;
    let mut events = manager.subscribe();

    manager.logout(&key).await?;

    assert_eq!(store.load(&key)?, None);
    assert_eq!(http.revoke_call_count(), 1);
    assert_eq!(events.recv().await?, TokenLifecycleEvent::LoggedOut { key });
    Ok(())
}

#[tokio::test]
async fn logout_with_revoke_network_failure_still_succeeds_locally() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(&key, &stored_credentials("access", "refresh", now(), 3600))?;
    let http = Arc::new(MockOAuthHttpClient::with_revoke(vec![Err(
        OAuthError::HttpTransport("network token=secret".to_string()),
    )]));
    let manager = manager(store.clone(), http.clone())?;

    manager.logout(&key).await?;

    assert_eq!(store.load(&key)?, None);
    assert_eq!(http.revoke_call_count(), 1);
    Ok(())
}

#[tokio::test]
async fn access_token_debug_redacts_token_content() -> Result<(), Box<dyn Error>> {
    let key = credential_key()?;
    let store = Arc::new(MemoryStore::default());
    store.store(
        &key,
        &stored_credentials("secret-access-token", "refresh", now(), 3600),
    )?;
    let manager = manager(store, Arc::new(MockOAuthHttpClient::default()))?;

    let debug = format!("{:?}", manager.ensure_fresh(&key).await?);

    assert!(!debug.contains("secret-access-token"));
    assert!(debug.contains("<redacted:19 bytes>"));
    Ok(())
}

#[test]
fn token_refresh_error_display_redacts_secrets() {
    let secret = "secret-refresh-token";
    let error = TokenRefreshError::from_oauth(OAuthError::TokenEndpointStatus {
        status: 500,
        error_code: None,
        message: secret.to_string(),
    });

    assert!(!error.to_string().contains(secret));
}

fn manager(
    store: Arc<dyn CredentialStore>,
    http: Arc<dyn OAuthHttpClient>,
) -> Result<TokenManager, Box<dyn Error>> {
    manager_with_policy(store, http, RefreshPolicy::default())
}

fn manager_with_policy(
    store: Arc<dyn CredentialStore>,
    http: Arc<dyn OAuthHttpClient>,
    policy: RefreshPolicy,
) -> Result<TokenManager, Box<dyn Error>> {
    Ok(TokenManager::new(store, http, test_config()?, policy))
}

fn test_config() -> Result<OAuthConfig, Box<dyn Error>> {
    Ok(OAuthConfig {
        auth_url: "https://auth.example.test/oauth/authorize".parse()?,
        token_url: "https://auth.example.test/oauth/token".parse()?,
        revoke_url: "https://auth.example.test/oauth/revoke".parse()?,
        client_id: "client-id".to_string(),
        scopes: vec!["openid".to_string(), "offline_access".to_string()],
        redirect_uri_template: "http://localhost:{port}/auth/callback".to_string(),
        callback_ports: vec![1455],
        callback_timeout: Duration::from_secs(300),
        originator: Some("taugentic-test".to_string()),
        allowed_workspace_id: None,
    })
}

fn credential_key() -> Result<CredentialKey, Box<dyn Error>> {
    Ok(CredentialKey::new(AuthProfileId::new(
        "auth-openai-chatgpt".to_string(),
    )?))
}

fn stored_credentials(
    access_token: &str,
    refresh_token: &str,
    stored_at: u64,
    expires_in: u64,
) -> StoredCredentials {
    StoredCredentials {
        token_set: TokenSet {
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
            id_token: None,
            expires_in: Some(expires_in),
            scope: Some("openid profile email offline_access".to_string()),
            api_access_token: None,
            account_info: None,
        },
        account: account(),
        stored_at,
        last_refreshed_at: None,
    }
}

fn token_set(access_token: &str, refresh_token: &str, expires_in: u64) -> TokenSet {
    TokenSet {
        access_token: access_token.to_string(),
        refresh_token: refresh_token.to_string(),
        id_token: None,
        expires_in: Some(expires_in),
        scope: Some("openid profile email offline_access".to_string()),
        api_access_token: None,
        account_info: None,
    }
}

fn account() -> AccountInfo {
    AccountInfo {
        account_id: "acct_123".to_string(),
        email: "user@example.com".to_string(),
        organization_id: None,
        plan_tier: Some("plus".to_string()),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[derive(Clone, Default)]
struct MemoryStore {
    credentials: Arc<Mutex<HashMap<CredentialKey, StoredCredentials>>>,
}

impl CredentialStore for MemoryStore {
    fn store(
        &self,
        key: &CredentialKey,
        creds: &StoredCredentials,
    ) -> Result<(), CredentialStoreError> {
        self.credentials
            .lock()
            .map_err(|_| CredentialStoreError::backend_unavailable("memory", "lock poisoned"))?
            .insert(key.clone(), creds.clone());
        Ok(())
    }

    fn load(&self, key: &CredentialKey) -> Result<Option<StoredCredentials>, CredentialStoreError> {
        Ok(self
            .credentials
            .lock()
            .map_err(|_| CredentialStoreError::backend_unavailable("memory", "lock poisoned"))?
            .get(key)
            .cloned())
    }

    fn delete(&self, key: &CredentialKey) -> Result<(), CredentialStoreError> {
        self.credentials
            .lock()
            .map_err(|_| CredentialStoreError::backend_unavailable("memory", "lock poisoned"))?
            .remove(key);
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "memory"
    }
}

#[derive(Default)]
struct MockOAuthHttpClient {
    refresh_responses: Mutex<VecDeque<Result<TokenSet, OAuthError>>>,
    revoke_responses: Mutex<VecDeque<Result<(), OAuthError>>>,
    refresh_calls: AtomicUsize,
    revoke_calls: AtomicUsize,
}

impl MockOAuthHttpClient {
    fn with_refresh(responses: Vec<Result<TokenSet, OAuthError>>) -> Self {
        Self {
            refresh_responses: Mutex::new(VecDeque::from(responses)),
            ..Self::default()
        }
    }

    fn with_revoke(responses: Vec<Result<(), OAuthError>>) -> Self {
        Self {
            revoke_responses: Mutex::new(VecDeque::from(responses)),
            ..Self::default()
        }
    }

    fn refresh_call_count(&self) -> usize {
        self.refresh_calls.load(Ordering::SeqCst)
    }

    fn revoke_call_count(&self) -> usize {
        self.revoke_calls.load(Ordering::SeqCst)
    }
}

impl OAuthHttpClient for MockOAuthHttpClient {
    fn post_form<'a>(
        &'a self,
        _url: &'a Url,
        _fields: &'a [crate::client::FormField],
    ) -> OAuthHttpFuture<'a> {
        Box::pin(async {
            Err(OAuthError::HttpTransport(
                "unexpected post_form".to_string(),
            ))
        })
    }

    fn refresh_token<'a>(
        &'a self,
        _token_url: &'a Url,
        _client_id: &'a str,
        _refresh_token: &'a str,
    ) -> OAuthTokenFuture<'a> {
        Box::pin(async move {
            self.refresh_calls.fetch_add(1, Ordering::SeqCst);
            self.refresh_responses
                .lock()
                .map_err(|_| OAuthError::HttpTransport("refresh lock poisoned".to_string()))?
                .pop_front()
                .unwrap_or_else(|| {
                    Err(OAuthError::HttpTransport(
                        "missing mock refresh response".to_string(),
                    ))
                })
        })
    }

    fn revoke_token<'a>(
        &'a self,
        _revoke_url: &'a Url,
        _client_id: &'a str,
        _token: &'a str,
    ) -> OAuthUnitFuture<'a> {
        Box::pin(async move {
            self.revoke_calls.fetch_add(1, Ordering::SeqCst);
            self.revoke_responses
                .lock()
                .map_err(|_| OAuthError::HttpTransport("revoke lock poisoned".to_string()))?
                .pop_front()
                .unwrap_or_else(|| {
                    Err(OAuthError::HttpTransport(
                        "missing mock revoke response".to_string(),
                    ))
                })
        })
    }
}
