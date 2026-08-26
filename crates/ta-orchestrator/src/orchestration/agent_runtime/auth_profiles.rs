use ta_protocol::wire::{
    AuthMethodId, AuthProfileId, AuthProfileLoginResult, AuthProfileLogoutResult, AuthProfileRef,
};

use crate::orchestration::{
    AgentRuntimeServiceError, agent_runtime::strategy_registry::StrategyRegistry,
};

pub(crate) async fn login_auth_profile(
    registry: &StrategyRegistry,
    auth_method_id: &AuthMethodId,
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLoginResult, AgentRuntimeServiceError> {
    registry.login(auth_method_id, auth_profile_id).await
}

pub(crate) async fn complete_auth_profile_login(
    registry: &StrategyRegistry,
    profile: &AuthProfileRef,
) -> Result<AuthProfileLoginResult, AgentRuntimeServiceError> {
    registry.complete_login(profile).await
}

pub(crate) async fn logout_auth_profile(
    registry: &StrategyRegistry,
    profile: &AuthProfileRef,
) -> Result<AuthProfileLogoutResult, AgentRuntimeServiceError> {
    registry.logout(profile).await
}
