use ta_protocol::wire::{
    AgentRuntimeModelId, AgentRuntimeSelection, AuthProfileId, GetAgentRuntimeQuery,
    RuntimeProfileId,
};
use ta_store::{AuthProfileRepository, InMemoryStore};

use super::AppService;

pub(crate) fn test_runtime_selection(
    service: &AppService<InMemoryStore>,
    runtime_profile_id: &str,
) -> AgentRuntimeSelection {
    let runtime_profile_id = RuntimeProfileId::new(runtime_profile_id).expect("runtime profile id");
    let snapshot = service
        .get_agent_runtime(&GetAgentRuntimeQuery {})
        .expect("agent runtime snapshot should load");
    let runtime_profile = snapshot
        .runtime_profiles
        .iter()
        .find(|profile| profile.id == runtime_profile_id)
        .expect("runtime profile should exist");
    let auth_profile_id = runtime_profile
        .auth_method_id
        .as_ref()
        .map(|auth_method_id| {
            let profile_id = format!("profile-{}-test", runtime_profile.provider_id.as_str());
            service
                .store
                .lock()
                .expect("app store should not be poisoned")
                .save_auth_profile(ta_store::connected_test_auth_profile(
                    &profile_id,
                    auth_method_id.as_str(),
                    runtime_profile.provider_id.as_str(),
                ))
                .expect("test auth profile should persist");
            AuthProfileId::new(profile_id).expect("auth profile id")
        });
    let model_id = if runtime_profile.auth_method_id.is_none() {
        "model-a"
    } else {
        "gpt-5.6-sol"
    };

    AgentRuntimeSelection {
        runtime_profile_id,
        auth_profile_id,
        model_id: Some(AgentRuntimeModelId::new(model_id).expect("model id")),
    }
}
