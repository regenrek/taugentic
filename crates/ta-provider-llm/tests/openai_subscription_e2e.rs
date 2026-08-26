mod support;

use std::sync::Arc;

use support::openai_mock::{
    MockBrowserLauncher, MockOpenAiServer, TestCredentialStore, TestResult, expect_lifecycle,
};
use ta_auth_openai::client::{OAuthHttpClient, ReqwestOAuthHttpClient};
use ta_auth_openai::{
    CredentialKey, CredentialStore, RefreshPolicy, TokenLifecycleEvent, TokenManager,
};
use ta_protocol::wire::{AuthProfileConnectionState, AuthProfileId, AuthProfileLoginMethod};
use ta_provider_llm::auth::openai::OpenAiAuth;
use ta_provider_llm::auth::openai_subscription::OpenAiSubscriptionAuth;
use ta_provider_llm::client::openai_responses::OpenAiResponsesClient;
use ta_provider_llm::client::{LlmClient, StreamMessage, StreamRequest};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn chatgpt_subscription_oauth_smoke_covers_login_refresh_retry_and_logout() -> TestResult {
    let server = MockOpenAiServer::start().await?;
    let store = Arc::new(TestCredentialStore::default());
    let key = CredentialKey::new(AuthProfileId::new("profile-test")?);
    let store_for_auth: Arc<dyn CredentialStore> = store.clone();
    let http: Arc<dyn OAuthHttpClient> = Arc::new(ReqwestOAuthHttpClient::default());
    let config = server.oauth_config();
    let manager = Arc::new(TokenManager::new(
        Arc::clone(&store_for_auth),
        Arc::clone(&http),
        config.clone(),
        RefreshPolicy::default(),
    ));
    let mut lifecycle = manager.subscribe();
    let (browser, browser_result) = MockBrowserLauncher::new();
    let auth = OpenAiSubscriptionAuth::from_parts(
        tokio::runtime::Handle::current(),
        store_for_auth,
        http,
        config,
        key.clone(),
        manager,
        browser.launcher(),
    );

    let login_result = auth.login().await?;
    assert_eq!(
        login_result.auth_profile.connection_state,
        AuthProfileConnectionState::PendingLogin
    );
    assert_eq!(
        login_result
            .challenge
            .as_ref()
            .map(|challenge| challenge.method),
        Some(AuthProfileLoginMethod::Browser)
    );
    browser_result.await??;
    assert_eq!(browser.launch_count(), 1);
    wait_for_stored_credentials(store.as_ref(), &key).await?;
    expect_lifecycle(&mut lifecycle, "login refreshed", |event| {
        matches!(event, TokenLifecycleEvent::Refreshed { key: event_key, .. } if event_key == &key)
    })
    .await?;
    assert_eq!(
        auth.current_state().connection_state,
        AuthProfileConnectionState::Connected
    );
    assert!(store.load(&key)?.is_some());
    assert_eq!(auth.bearer().await?, "mock-api-access");

    let client = OpenAiResponsesClient::with_auth_base_url_override_for_test(
        server.base_url("/v1"),
        OpenAiAuth::Subscription { auth: auth.clone() },
        "gpt-test",
    )?;
    let stream = client
        .start_stream(
            StreamRequest {
                model: String::new(),
                messages: vec![StreamMessage::user("say hi")],
                tools: Vec::new(),
                provider_session_id: None,
            },
            CancellationToken::new(),
        )
        .await;
    assert!(stream.is_ok());
    assert_eq!(auth.bearer().await?, "fresh-api-access");
    expect_lifecycle(&mut lifecycle, "refreshed", |event| {
        matches!(event, TokenLifecycleEvent::Refreshed { key: event_key, .. } if event_key == &key)
    })
    .await?;

    let logout = auth.logout().await?;
    assert!(logout.disconnected);
    assert!(store.load(&key)?.is_none());
    assert_eq!(
        auth.current_state().connection_state,
        AuthProfileConnectionState::LoggedOut
    );
    expect_lifecycle(&mut lifecycle, "logged out", |event| {
        matches!(event, TokenLifecycleEvent::LoggedOut { key: event_key } if event_key == &key)
    })
    .await?;

    let snapshot = server.snapshot();
    assert_eq!(
        snapshot.token_grants,
        vec![
            "authorization_code".to_string(),
            "urn:ietf:params:oauth:grant-type:token-exchange".to_string(),
            "refresh_token".to_string()
        ]
    );
    assert_eq!(
        snapshot.response_bearers,
        vec![
            "Bearer mock-api-access".to_string(),
            "Bearer fresh-api-access".to_string()
        ]
    );
    assert_eq!(snapshot.revoke_count, 1);
    assert!(snapshot.failures.is_empty(), "{:?}", snapshot.failures);

    Ok(())
}

async fn wait_for_stored_credentials(
    store: &TestCredentialStore,
    key: &CredentialKey,
) -> TestResult {
    for _ in 0..50 {
        if store.load(key)?.is_some() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    Err("timed out waiting for stored OpenAI subscription credentials".into())
}
