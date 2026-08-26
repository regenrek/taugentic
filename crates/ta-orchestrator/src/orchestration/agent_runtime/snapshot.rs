use ta_protocol::wire::{
    AgentRuntimeSnapshot, AgentRuntimeStrategyInfo, AuthMethodRef, AuthProfileState,
};

use crate::orchestration::agent_runtime::service::{AgentRuntimeServiceError, AgentRuntimeState};

pub(crate) fn build_snapshot(
    state: &AgentRuntimeState,
    providers: Vec<AgentRuntimeStrategyInfo>,
    auth_methods: Vec<AuthMethodRef>,
    auth_profiles: Vec<AuthProfileState>,
) -> Result<AgentRuntimeSnapshot, AgentRuntimeServiceError> {
    Ok(AgentRuntimeSnapshot {
        providers,
        auth_methods,
        auth_profiles,
        runtime_profiles: state.runtime_profiles.clone(),
        runtime_extensions: state.runtime_extensions.clone(),
    })
}
