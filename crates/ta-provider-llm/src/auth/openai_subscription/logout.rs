use ta_protocol::wire::AuthProfileLogoutResult;

use super::{OpenAiSubscriptionAuth, lifecycle, profile};
use crate::error::LlmClientError;

impl OpenAiSubscriptionAuth {
    pub async fn logout(&self) -> Result<AuthProfileLogoutResult, LlmClientError> {
        self.cancel_pending_login().await;
        self.inner
            .manager
            .logout(self.key())
            .await
            .map_err(|error| {
                lifecycle::record_refresh_error(self, &error);
                super::map_refresh_error(error)
            })?;
        profile::record_logged_out(self);
        Ok(AuthProfileLogoutResult {
            auth_profile_id: ta_protocol::wire::AuthProfileId::new(
                super::OPENAI_CHATGPT_SUBSCRIPTION_AUTH_PROFILE_ID,
            )
            .expect("OpenAI ChatGPT auth profile id"),
            disconnected: true,
        })
    }
}
