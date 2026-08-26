use ta_protocol::wire::{
    AgentRuntimeModelId, AgentRuntimeSelection, AuthProfileId, RuntimeProfileId,
};

#[test]
fn runtime_selection_carries_the_complete_daemon_validated_route() {
    let selection = AgentRuntimeSelection {
        runtime_profile_id: RuntimeProfileId::new("runtime-test").expect("runtime profile id"),
        auth_profile_id: Some(AuthProfileId::new("profile-test").expect("auth profile id")),
        model_id: Some(AgentRuntimeModelId::new("model-test").expect("model id")),
    };

    assert_eq!(selection.runtime_profile_id.as_str(), "runtime-test");
    assert_eq!(
        selection
            .auth_profile_id
            .as_ref()
            .map(AuthProfileId::as_str),
        Some("profile-test")
    );
    assert_eq!(
        selection.model_id.as_ref().map(AgentRuntimeModelId::as_str),
        Some("model-test")
    );
}
