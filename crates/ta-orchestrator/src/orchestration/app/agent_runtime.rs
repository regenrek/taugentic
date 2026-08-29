use ta_store::PersistenceStore;

use crate::{
    AuthProfileLoginResult, AuthProfileLogoutResult, DaemonAgentRuntimeAuthLoginCompleteParams,
    DaemonAgentRuntimeAuthLoginParams, DaemonAgentRuntimeAuthLogoutParams,
    DaemonAgentRuntimeAuthProfilePreferencesSetParams, DaemonAgentRuntimePatchProfileParams,
    DaemonAgentRuntimeSetExtensionEnabledParams, GetAgentRuntimeQuery,
};

use super::{AppService, AppServiceError};

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub fn get_agent_runtime(
        &self,
        query: &GetAgentRuntimeQuery,
    ) -> Result<crate::AgentRuntimeSnapshot, AppServiceError> {
        self.agent_runtime
            .snapshot(query)
            .map_err(AppServiceError::from)
    }

    pub fn patch_agent_runtime_profile(
        &self,
        params: &DaemonAgentRuntimePatchProfileParams,
    ) -> Result<crate::AgentRuntimeSnapshot, AppServiceError> {
        self.agent_runtime
            .patch_profile(params)
            .map_err(AppServiceError::from)
    }

    pub async fn login_agent_runtime_auth_profile(
        &self,
        params: &DaemonAgentRuntimeAuthLoginParams,
    ) -> Result<AuthProfileLoginResult, AppServiceError> {
        self.agent_runtime
            .login_auth_profile(params)
            .await
            .map_err(AppServiceError::from)
    }

    pub fn replace_agent_runtime_auth_profile_preferences(
        &self,
        params: &DaemonAgentRuntimeAuthProfilePreferencesSetParams,
    ) -> Result<crate::AgentRuntimeSnapshot, AppServiceError> {
        self.agent_runtime
            .replace_auth_profile_preferences(params)
            .map_err(AppServiceError::from)
    }

    pub async fn logout_agent_runtime_auth_profile(
        &self,
        params: &DaemonAgentRuntimeAuthLogoutParams,
    ) -> Result<AuthProfileLogoutResult, AppServiceError> {
        self.agent_runtime
            .logout_auth_profile(params)
            .await
            .map_err(AppServiceError::from)
    }

    pub async fn complete_agent_runtime_auth_profile_login(
        &self,
        params: &DaemonAgentRuntimeAuthLoginCompleteParams,
    ) -> Result<AuthProfileLoginResult, AppServiceError> {
        self.agent_runtime
            .complete_auth_profile_login(params)
            .await
            .map_err(AppServiceError::from)
    }

    pub fn set_agent_runtime_extension_enabled(
        &self,
        params: &DaemonAgentRuntimeSetExtensionEnabledParams,
    ) -> Result<crate::AgentRuntimeSnapshot, AppServiceError> {
        self.agent_runtime
            .set_extension_enabled(params)
            .map_err(AppServiceError::from)
    }
}
