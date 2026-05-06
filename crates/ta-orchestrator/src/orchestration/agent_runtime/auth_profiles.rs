use ta_protocol::wire::{AuthProfileId, AuthProfileLoginResult, AuthProfileLogoutResult};

use crate::orchestration::{
    AgentRuntimeServiceError, agent_runtime::strategy_registry::StrategyRegistry,
};

pub(crate) async fn login_auth_profile(
    registry: &StrategyRegistry,
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLoginResult, AgentRuntimeServiceError> {
    registry.login(auth_profile_id).await
}

pub(crate) async fn logout_auth_profile(
    registry: &StrategyRegistry,
    auth_profile_id: &AuthProfileId,
) -> Result<AuthProfileLogoutResult, AgentRuntimeServiceError> {
    registry.logout(auth_profile_id).await
}
