use ta_protocol::wire::{
    AgentRuntimeSelection, AgentRuntimeSnapshot, AgentRuntimeStrategyInfo, AuthProfileState,
};

use crate::orchestration::agent_runtime::service::{AgentRuntimeServiceError, AgentRuntimeState};

pub(crate) fn build_snapshot(
    state: &AgentRuntimeState,
    providers: Vec<AgentRuntimeStrategyInfo>,
    auth_profiles: Vec<AuthProfileState>,
) -> Result<AgentRuntimeSnapshot, AgentRuntimeServiceError> {
    let runtime_profile_id = state
        .selection
        .clone()
        .ok_or(AgentRuntimeServiceError::NoRuntimeProviderConfigured)?;
    Ok(AgentRuntimeSnapshot {
        selection: AgentRuntimeSelection { runtime_profile_id },
        providers,
        auth_profiles,
        runtime_profiles: state.runtime_profiles.clone(),
        runtime_extensions: state.runtime_extensions.clone(),
    })
}
