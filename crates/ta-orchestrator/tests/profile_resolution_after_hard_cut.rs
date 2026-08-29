use ta_protocol::wire::{
    AgentRuntimeStrategyId, AuthMethodId, AuthProfileConnectionState, AuthProfileId,
    AuthProfileManagementMode, AuthProfilePreferences, AuthProfileRef, AuthProfileState,
    AuthProfileUsage,
};
use ta_store::{AuthProfileProjection, AuthProfileRepository, InMemoryStore};

#[test]
fn durable_auth_profiles_keep_method_and_provider_ownership() {
    let mut store = InMemoryStore::current();
    let projection = AuthProfileProjection {
        profile: AuthProfileState {
            profile: AuthProfileRef {
                id: AuthProfileId::new("profile-test").expect("auth profile id"),
                auth_method_id: AuthMethodId::new("method-test").expect("auth method id"),
                provider_id: AgentRuntimeStrategyId::new("provider-test").expect("provider id"),
                display_name: "Test profile".to_string(),
                account_hint: None,
                plan_tier: None,
            },
            preferences: AuthProfilePreferences {
                label: "Codex ChatGPT".to_string(),
                order: 0,
                is_default: false,
            },
            usage: AuthProfileUsage::Unavailable,
            exhaustion: None,
            connection_state: AuthProfileConnectionState::Connected,
            last_error: None,
            management_mode: AuthProfileManagementMode::Interactive,
            can_login: false,
            can_logout: true,
            platform_org_linked: None,
            setup_steps: Vec::new(),
            action: None,
            methods: Vec::new(),
        },
        external_account_id: None,
    };
    store
        .save_auth_profile(projection.clone())
        .expect("save auth profile");

    assert_eq!(
        store
            .auth_profile(&projection.profile.profile.id)
            .expect("load auth profile"),
        Some(projection),
    );
}
