use ta_store::PersistenceStore;

use crate::{
    AuthProfileLoginResult, AuthProfileLogoutResult, DaemonAgentRuntimeAuthLoginParams,
    DaemonAgentRuntimeAuthLogoutParams, DaemonAgentRuntimePatchProfileParams,
    DaemonAgentRuntimeSelectProfileParams, DaemonAgentRuntimeSetExtensionEnabledParams,
    DaemonAgentRuntimeTestLocalEndpointParams, GetAgentRuntimeQuery, LocalModelEndpointTestResult,
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

    pub fn select_agent_runtime_profile(
        &self,
        params: &DaemonAgentRuntimeSelectProfileParams,
    ) -> Result<crate::AgentRuntimeSnapshot, AppServiceError> {
        self.agent_runtime
            .select_profile(params)
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

    pub async fn logout_agent_runtime_auth_profile(
        &self,
        params: &DaemonAgentRuntimeAuthLogoutParams,
    ) -> Result<AuthProfileLogoutResult, AppServiceError> {
        self.agent_runtime
            .logout_auth_profile(params)
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

    pub async fn test_local_model_endpoint(
        &self,
        params: &DaemonAgentRuntimeTestLocalEndpointParams,
    ) -> Result<LocalModelEndpointTestResult, AppServiceError> {
        self.agent_runtime
            .test_local_endpoint(params)
            .await
            .map_err(AppServiceError::from)
    }
}
