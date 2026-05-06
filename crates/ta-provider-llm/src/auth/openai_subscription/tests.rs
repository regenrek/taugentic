use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ta_auth_openai::browser::BrowserLaunch;
use ta_auth_openai::client::{FormField, OAuthHttpClient, OAuthHttpFuture, OAuthHttpResponse};
use ta_auth_openai::{
    AccountInfo, CredentialKey, CredentialStore, CredentialStoreError, OAuthConfig, RefreshPolicy,
    StoredCredentials, TokenLifecycleEvent, TokenManager, TokenSet,
};
use ta_protocol::wire::{
    AuthProfileConnectionState, AuthProfileId, AuthProfileLoginChallenge, AuthProfileLoginMethod,
};
use url::Url;

use super::{OpenAiSubscriptionAuth, profile};

#[tokio::test]
async fn current_state_tracks_credentials_and_reauth() {
    let store = Arc::new(TestStore::default());
    let auth = test_auth(Arc::clone(&store), Arc::new(ScriptedHttp::default()));

    assert_eq!(
        auth.current_state().connection_state,
        AuthProfileConnectionState::LoggedOut
    );

    store.replace(Some(stored_credentials("existing-access", "refresh")));
    let connected = auth.current_state();
    assert_eq!(
        connected.connection_state,
        AuthProfileConnectionState::Connected
    );
    assert_eq!(connected.platform_org_linked, Some(false));

    profile::record_needs_reauth(&auth, "refresh token rejected".to_string());
    let state = auth.current_state();
    assert_eq!(state.connection_state, AuthProfileConnectionState::Error);
    assert!(
        state
            .last_error
            .as_deref()
            .is_some_and(|message| message.contains("needs re-authentication"))
    );
}

#[tokio::test]
async fn bearer_uses_stored_subscription_api_token() {
    let store = Arc::new(TestStore::default());
    store.replace(Some(stored_credentials_with_api_token(
        "oauth-access",
        "api-access",
        "refresh",
    )));
    let auth = test_auth(store, Arc::new(ScriptedHttp::default()));

    let bearer = auth.bearer().await.expect("bearer token");

    assert_eq!(bearer, "api-access");
}

#[tokio::test]
async fn force_refresh_updates_bearer() {
    let store = Arc::new(TestStore::default());
    store.replace(Some(stored_credentials("old-access", "refresh")));
    let http = Arc::new(ScriptedHttp::with_responses([OAuthHttpResponse {
        status: 200,
        body:
            r#"{"access_token":"fresh-access","refresh_token":"fresh-refresh","expires_in":3600}"#
                .to_string(),
    }]));
    let auth = test_auth(Arc::clone(&store), http.clone());

    let bearer = auth.force_refresh().await.expect("force refresh");

    assert_eq!(bearer, "fresh-access");
    assert_eq!(http.post_count(), 1);
}

#[tokio::test]
async fn login_flow_persists_credentials_and_connects_profile() {
    let store = Arc::new(TestStore::default());
    let http = Arc::new(ScriptedHttp::with_responses([
        OAuthHttpResponse {
            status: 200,
            body: format!(
                r#"{{"access_token":"oauth-access","refresh_token":"refresh","id_token":"{}","expires_in":3600}}"#,
                id_token_without_organization()
            ),
        },
        OAuthHttpResponse {
            status: 401,
            body: r#"{"error":"invalid_request","error_description":"Invalid ID token: missing organization_id"}"#
                .to_string(),
        },
    ]));
    let captured_authorize_url = Arc::new(Mutex::new(None::<Url>));
    let launcher_url = Arc::clone(&captured_authorize_url);
    let auth = test_auth_with_launcher(
        Arc::clone(&store),
        Arc::clone(&http),
        Arc::new(move |url: &Url| {
            *launcher_url.lock().expect("launcher url lock") = Some(url.clone());
            BrowserLaunch::Opened
        }),
    );

    let result = tokio::time::timeout(Duration::from_secs(1), auth.login())
        .await
        .expect("login should return before OAuth callback")
        .expect("login result");

    assert_eq!(
        result.auth_profile.connection_state,
        AuthProfileConnectionState::PendingLogin
    );
    let challenge = result.challenge.expect("browser login challenge");
    assert_eq!(challenge.method, AuthProfileLoginMethod::Browser);
    assert!(store.load(auth.key()).expect("load credentials").is_none());

    let authorize_url = challenge
        .authorize_url
        .as_deref()
        .map(Url::parse)
        .transpose()
        .expect("authorize url parses")
        .expect("authorize url");
    assert_eq!(
        wait_for_authorize_url(captured_authorize_url).await,
        authorize_url
    );
    let redirect_uri = query_value(&authorize_url, "redirect_uri");
    let state = query_value(&authorize_url, "state");
    let callback = format!("{redirect_uri}?code=test-code&state={state}");
    reqwest::get(callback).await.expect("callback request");

    wait_for_stored_credentials(&store).await;
    let state = auth.current_state();
    assert_eq!(
        state.connection_state,
        AuthProfileConnectionState::Connected
    );
    assert_eq!(state.platform_org_linked, Some(false));
    let credentials = store
        .load(auth.key())
        .expect("load credentials")
        .expect("stored credentials");
    assert_eq!(credentials.token_set.api_access_token, None);
    assert_eq!(credentials.account.organization_id, None);
    assert_eq!(http.post_count(), 2);
}

#[tokio::test]
async fn login_flow_marks_platform_org_linked_when_id_token_has_organization() {
    let store = Arc::new(TestStore::default());
    let http = Arc::new(ScriptedHttp::with_responses([
        OAuthHttpResponse {
            status: 200,
            body: format!(
                r#"{{"access_token":"oauth-access","refresh_token":"refresh","id_token":"{}","expires_in":3600}}"#,
                id_token_with_organization()
            ),
        },
        OAuthHttpResponse {
            status: 200,
            body: r#"{"access_token":"api-access"}"#.to_string(),
        },
    ]));
    let auth = test_auth(Arc::clone(&store), http.clone());

    let result = auth.login().await.expect("manual login result");
    let challenge = result.challenge.expect("manual challenge");
    let authorize_url = challenge_authorize_url(&challenge);
    let callback = format!(
        "{}?code=test-code&state={}",
        query_value(&authorize_url, "redirect_uri"),
        query_value(&authorize_url, "state")
    );
    reqwest::get(callback).await.expect("callback request");

    wait_for_stored_credentials(&store).await;
    let state = auth.current_state();
    assert_eq!(
        state.connection_state,
        AuthProfileConnectionState::Connected
    );
    assert_eq!(state.platform_org_linked, Some(true));
    let credentials = store
        .load(auth.key())
        .expect("load credentials")
        .expect("stored credentials");
    assert_eq!(
        credentials.account.organization_id.as_deref(),
        Some("org_123")
    );
    assert_eq!(
        credentials.token_set.api_access_token.as_deref(),
        Some("api-access")
    );
    assert_eq!(http.post_count(), 2);
}

#[tokio::test]
async fn login_flow_returns_manual_browser_challenge_when_launcher_fails() {
    let store = Arc::new(TestStore::default());
    let http = Arc::new(ScriptedHttp::with_responses([OAuthHttpResponse {
        status: 200,
        body: r#"{"access_token":"oauth-access","refresh_token":"refresh","expires_in":3600}"#
            .to_string(),
    }]));
    let auth = test_auth(Arc::clone(&store), http);

    let result = auth.login().await.expect("manual login result");

    assert_eq!(
        result.auth_profile.connection_state,
        AuthProfileConnectionState::PendingLogin
    );
    let challenge = result.challenge.expect("manual login challenge");
    assert_eq!(challenge.method, AuthProfileLoginMethod::Manual);
    let manual_url = challenge
        .manual_browser_url
        .as_deref()
        .expect("manual browser url");
    let authorize_url = Url::parse(manual_url).expect("manual url parses");
    let redirect_uri = query_value(&authorize_url, "redirect_uri");
    let state = query_value(&authorize_url, "state");
    let callback = format!("{redirect_uri}?code=test-code&state={state}");
    reqwest::get(callback).await.expect("callback request");

    wait_for_stored_credentials(&store).await;
    assert_eq!(
        auth.current_state().connection_state,
        AuthProfileConnectionState::Connected
    );
}

#[tokio::test]
async fn logout_cancels_pending_login_completion() {
    let store = Arc::new(TestStore::default());
    let http = Arc::new(ScriptedHttp::with_responses([OAuthHttpResponse {
        status: 200,
        body: r#"{"access_token":"oauth-access","refresh_token":"refresh","expires_in":3600}"#
            .to_string(),
    }]));
    let auth = test_auth(Arc::clone(&store), http);

    let result = auth.login().await.expect("manual login result");
    let challenge = result.challenge.expect("manual challenge");
    let manual_url = challenge
        .manual_browser_url
        .as_deref()
        .expect("manual browser url");
    let authorize_url = Url::parse(manual_url).expect("manual url parses");
    let redirect_uri = query_value(&authorize_url, "redirect_uri");
    let state = query_value(&authorize_url, "state");

    auth.logout()
        .await
        .expect("logout should cancel pending login");
    let callback = format!("{redirect_uri}?code=test-code&state={state}");
    let _ = reqwest::get(callback).await;

    assert_credentials_absent_for(&store, Duration::from_millis(200)).await;
    assert_eq!(
        auth.current_state().connection_state,
        AuthProfileConnectionState::LoggedOut
    );
}

#[tokio::test]
async fn relogin_cancels_previous_pending_login_and_reuses_callback_port() {
    let port = unused_callback_port();
    let store = Arc::new(TestStore::default());
    let http = Arc::new(ScriptedHttp::with_responses([OAuthHttpResponse {
        status: 200,
        body: r#"{"access_token":"oauth-access","refresh_token":"refresh","expires_in":3600}"#
            .to_string(),
    }]));
    let mut config = test_config();
    config.callback_ports = vec![port];
    config.callback_timeout = Duration::from_secs(30);
    let auth = test_auth_with_config(Arc::clone(&store), http, config);

    let first = auth.login().await.expect("first login");
    let first_url = challenge_authorize_url(&first.challenge.expect("first challenge"));
    assert_eq!(callback_port(&first_url), port);
    assert_eq!(auth.pending_login_is_finished_for_test(), Some(false));
    assert!(!can_bind_callback_port(port));

    let second = auth.login().await.expect("second login");
    let second_challenge = second.challenge.expect("second challenge");
    let second_url = challenge_authorize_url(&second_challenge);
    assert_eq!(callback_port(&second_url), port);
    assert_eq!(auth.pending_login_is_finished_for_test(), Some(false));
    assert!(!can_bind_callback_port(port));

    let callback = format!(
        "{}?code=test-code&state={}",
        query_value(&second_url, "redirect_uri"),
        query_value(&second_url, "state")
    );
    reqwest::get(callback)
        .await
        .expect("second callback request");

    wait_for_stored_credentials(&store).await;
    assert_eq!(
        auth.current_state().connection_state,
        AuthProfileConnectionState::Connected
    );
}

#[tokio::test]
async fn dropping_auth_aborts_pending_login_and_releases_callback_port() {
    let port = unused_callback_port();
    let store = Arc::new(TestStore::default());
    let http = Arc::new(ScriptedHttp::default());
    let mut config = test_config();
    config.callback_ports = vec![port];
    config.callback_timeout = Duration::from_secs(30);

    {
        let auth = test_auth_with_config(store, http, config);
        let _ = auth.current_state();
        let result = auth.login().await.expect("pending login");
        let challenge = result.challenge.expect("challenge");
        assert_eq!(callback_port(&challenge_authorize_url(&challenge)), port);
        assert!(!can_bind_callback_port(port));
    }

    wait_for_callback_port_released(port).await;
}

#[tokio::test]
async fn completion_panic_emits_login_failed_lifecycle_event() {
    let store = Arc::new(TestStore::default());
    let store_for_auth: Arc<dyn CredentialStore> = store.clone();
    let http: Arc<dyn OAuthHttpClient> = Arc::new(PanicHttp);
    let config = test_config();
    let key = credential_key();
    let manager = Arc::new(TokenManager::new(
        Arc::clone(&store_for_auth),
        Arc::clone(&http),
        config.clone(),
        RefreshPolicy::default(),
    ));
    let mut lifecycle = manager.subscribe();
    let auth = OpenAiSubscriptionAuth::from_parts(
        tokio::runtime::Handle::current(),
        store_for_auth,
        http,
        config,
        key.clone(),
        manager,
        Arc::new(|url: &Url| BrowserLaunch::Manual {
            authorize_url: url.clone(),
            reason: "test launcher disabled".to_string(),
        }),
    );

    let result = auth.login().await.expect("manual login result");
    let authorize_url = challenge_authorize_url(&result.challenge.expect("manual challenge"));
    let callback = format!(
        "{}?code=test-code&state={}",
        query_value(&authorize_url, "redirect_uri"),
        query_value(&authorize_url, "state")
    );
    reqwest::get(callback).await.expect("callback request");

    let event = wait_for_login_failed(&mut lifecycle).await;
    assert_eq!(
        event,
        TokenLifecycleEvent::LoginFailed {
            key,
            reason: "OpenAI ChatGPT OAuth login completion task panicked".to_string(),
        }
    );
    let state = auth.current_state();
    assert_eq!(
        state.connection_state,
        AuthProfileConnectionState::LoggedOut
    );
    assert_eq!(
        state.last_error.as_deref(),
        Some("OpenAI ChatGPT OAuth login completion task panicked")
    );
}

fn test_auth(store: Arc<TestStore>, http: Arc<ScriptedHttp>) -> OpenAiSubscriptionAuth {
    test_auth_with_launcher(
        store,
        http,
        Arc::new(|url: &Url| BrowserLaunch::Manual {
            authorize_url: url.clone(),
            reason: "test launcher disabled".to_string(),
        }),
    )
}

fn test_auth_with_launcher(
    store: Arc<TestStore>,
    http: Arc<ScriptedHttp>,
    launcher: Arc<dyn Fn(&Url) -> BrowserLaunch + Send + Sync>,
) -> OpenAiSubscriptionAuth {
    test_auth_with_launcher_and_config(store, http, launcher, test_config())
}

fn test_auth_with_config(
    store: Arc<TestStore>,
    http: Arc<ScriptedHttp>,
    config: OAuthConfig,
) -> OpenAiSubscriptionAuth {
    test_auth_with_launcher_and_config(
        store,
        http,
        Arc::new(|url: &Url| BrowserLaunch::Manual {
            authorize_url: url.clone(),
            reason: "test launcher disabled".to_string(),
        }),
        config,
    )
}

fn test_auth_with_launcher_and_config(
    store: Arc<TestStore>,
    http: Arc<ScriptedHttp>,
    launcher: Arc<dyn Fn(&Url) -> BrowserLaunch + Send + Sync>,
    config: OAuthConfig,
) -> OpenAiSubscriptionAuth {
    let store: Arc<dyn CredentialStore> = store;
    let http: Arc<dyn OAuthHttpClient> = http;
    let key = CredentialKey::new(AuthProfileId::new("auth-openai-chatgpt").expect("auth id"));
    let manager = Arc::new(TokenManager::new(
        Arc::clone(&store),
        Arc::clone(&http),
        config.clone(),
        RefreshPolicy::default(),
    ));
    OpenAiSubscriptionAuth::from_parts(
        tokio::runtime::Handle::current(),
        store,
        http,
        config,
        key,
        manager,
        launcher,
    )
}

fn test_config() -> OAuthConfig {
    OAuthConfig {
        auth_url: Url::parse("https://auth.example.test/oauth/authorize").expect("auth url"),
        token_url: Url::parse("https://auth.example.test/oauth/token").expect("token url"),
        revoke_url: Url::parse("https://auth.example.test/oauth/revoke").expect("revoke url"),
        client_id: "test-client".to_string(),
        scopes: vec!["openid".to_string(), "offline_access".to_string()],
        redirect_uri_template: "http://localhost:{port}/auth/callback".to_string(),
        callback_ports: vec![0],
        callback_timeout: Duration::from_secs(5),
        originator: None,
        allowed_workspace_id: None,
    }
}

fn stored_credentials(access_token: &str, refresh_token: &str) -> StoredCredentials {
    stored_credentials_with_api_token(access_token, access_token, refresh_token)
}

fn stored_credentials_with_api_token(
    access_token: &str,
    api_access_token: &str,
    refresh_token: &str,
) -> StoredCredentials {
    StoredCredentials {
        token_set: TokenSet {
            access_token: access_token.to_string(),
            refresh_token: refresh_token.to_string(),
            id_token: None,
            expires_in: Some(3600),
            scope: Some("openid offline_access".to_string()),
            api_access_token: Some(api_access_token.to_string()),
            account_info: None,
        },
        account: AccountInfo {
            account_id: "acct_test".to_string(),
            email: "user@example.test".to_string(),
            organization_id: None,
            plan_tier: Some("plus".to_string()),
        },
        stored_at: now_unix_seconds(),
        last_refreshed_at: None,
    }
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs()
}

async fn wait_for_authorize_url(captured: Arc<Mutex<Option<Url>>>) -> Url {
    for _ in 0..50 {
        if let Some(url) = captured.lock().expect("authorize url lock").clone() {
            return url;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("authorize URL was not captured");
}

async fn wait_for_stored_credentials(store: &TestStore) {
    for _ in 0..50 {
        if store
            .load(&credential_key())
            .expect("load credentials")
            .is_some()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("credentials were not stored");
}

async fn assert_credentials_absent_for(store: &TestStore, duration: Duration) {
    let deadline = tokio::time::Instant::now() + duration;
    while tokio::time::Instant::now() < deadline {
        assert!(
            store
                .load(&credential_key())
                .expect("load credentials")
                .is_none(),
            "credentials should not be stored"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn challenge_authorize_url(challenge: &AuthProfileLoginChallenge) -> Url {
    challenge
        .authorize_url
        .as_deref()
        .or(challenge.manual_browser_url.as_deref())
        .map(Url::parse)
        .transpose()
        .expect("challenge url parses")
        .expect("challenge url")
}

fn callback_port(authorize_url: &Url) -> u16 {
    Url::parse(&query_value(authorize_url, "redirect_uri"))
        .expect("redirect uri parses")
        .port()
        .expect("redirect uri port")
}

fn unused_callback_port() -> u16 {
    let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .expect("bind ephemeral callback port");
    listener.local_addr().expect("local addr").port()
}

fn can_bind_callback_port(port: u16) -> bool {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).is_ok()
}

async fn wait_for_callback_port_released(port: u16) {
    for _ in 0..50 {
        if can_bind_callback_port(port) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("callback port {port} was not released");
}

async fn wait_for_login_failed(
    lifecycle: &mut tokio::sync::broadcast::Receiver<TokenLifecycleEvent>,
) -> TokenLifecycleEvent {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let event = lifecycle.recv().await.expect("lifecycle event");
            if matches!(event, TokenLifecycleEvent::LoginFailed { .. }) {
                return event;
            }
        }
    })
    .await
    .expect("login failed lifecycle event")
}

fn credential_key() -> CredentialKey {
    CredentialKey::new(AuthProfileId::new("auth-openai-chatgpt").expect("auth id"))
}

fn query_value(url: &Url, key: &str) -> String {
    url.query_pairs()
        .find_map(|(name, value)| (name == key).then(|| value.into_owned()))
        .unwrap_or_else(|| panic!("missing query value {key}"))
}

fn id_token_with_organization() -> &'static str {
    "e30.eyJleHAiOjE4MDAwMDAwMDAsImVtYWlsIjoidXNlckBleGFtcGxlLmNvbSIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhY2NfMTIzIiwiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIiwiY2hhdGdwdF9hY2NvdW50X2lzX2ZlZHJhbXAiOmZhbHNlLCJvcmdhbml6YXRpb25faWQiOiJvcmdfMTIzIn19.sig"
}

fn id_token_without_organization() -> &'static str {
    "e30.eyJleHAiOjE4MDAwMDAwMDAsImVtYWlsIjoidXNlckBleGFtcGxlLmNvbSIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhY2NfMTIzIiwiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIiwiY2hhdGdwdF9hY2NvdW50X2lzX2ZlZHJhbXAiOmZhbHNlfX0.sig"
}

#[derive(Default)]
struct TestStore {
    credentials: Mutex<Option<StoredCredentials>>,
}

impl TestStore {
    fn replace(&self, credentials: Option<StoredCredentials>) {
        *self.credentials.lock().expect("store lock") = credentials;
    }
}

impl CredentialStore for TestStore {
    fn store(
        &self,
        _key: &CredentialKey,
        credentials: &StoredCredentials,
    ) -> Result<(), CredentialStoreError> {
        self.replace(Some(credentials.clone()));
        Ok(())
    }

    fn load(
        &self,
        _key: &CredentialKey,
    ) -> Result<Option<StoredCredentials>, CredentialStoreError> {
        Ok(self.credentials.lock().expect("store lock").clone())
    }

    fn delete(&self, _key: &CredentialKey) -> Result<(), CredentialStoreError> {
        self.replace(None);
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "test"
    }
}

struct PanicHttp;

impl OAuthHttpClient for PanicHttp {
    fn post_form<'a>(&'a self, _url: &'a Url, _fields: &'a [FormField]) -> OAuthHttpFuture<'a> {
        Box::pin(async move {
            panic!("simulated completion panic");
        })
    }
}

#[derive(Default)]
struct ScriptedHttp {
    responses: Mutex<VecDeque<OAuthHttpResponse>>,
    post_count: AtomicUsize,
}

impl ScriptedHttp {
    fn with_responses<const N: usize>(responses: [OAuthHttpResponse; N]) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from(responses)),
            post_count: AtomicUsize::new(0),
        }
    }

    fn post_count(&self) -> usize {
        self.post_count.load(Ordering::SeqCst)
    }
}

impl OAuthHttpClient for ScriptedHttp {
    fn post_form<'a>(&'a self, _url: &'a Url, _fields: &'a [FormField]) -> OAuthHttpFuture<'a> {
        Box::pin(async move {
            self.post_count.fetch_add(1, Ordering::SeqCst);
            self.responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .ok_or_else(|| ta_auth_openai::OAuthError::HttpTransport("no response".to_string()))
        })
    }
}
