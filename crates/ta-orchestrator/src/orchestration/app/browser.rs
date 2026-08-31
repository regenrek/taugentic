use super::{AppService, AppServiceError};
use ta_policy::{decide_browser_action, deny_browser_unauthorized};
use ta_protocol::wire::{
    BrowserActionRequest, BrowserActionResult, BrowserClearDataRequest, BrowserProfile,
    BrowserProfileId, BrowserProfileRequest, BrowserProfileResult,
};
use ta_store::{BrowserProfileRepository, PersistenceStore};
use uuid::Uuid;

impl<S> AppService<S>
where
    S: PersistenceStore + Send,
{
    pub(crate) fn browser_profile(
        &self,
        owner_principal_id: &str,
        _: &BrowserProfileRequest,
    ) -> Result<BrowserProfileResult, AppServiceError> {
        let mut store = self.store.lock().expect("app store should not be poisoned");
        if let Some(profile) = store.browser_profile(owner_principal_id)? {
            return Ok(BrowserProfileResult { profile });
        }
        let profile = BrowserProfile {
            id: BrowserProfileId::new(Uuid::new_v4().to_string())
                .map_err(|_| AppServiceError::BrowserProfileInvalid)?,
        };
        store.save_browser_profile(owner_principal_id, profile.clone())?;
        Ok(BrowserProfileResult { profile })
    }
    pub(crate) fn decide_browser_action(
        &self,
        owner: &str,
        request: &BrowserActionRequest,
    ) -> Result<BrowserActionResult, AppServiceError> {
        let profile = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .browser_profile(owner)?;
        if profile.as_ref().map(|profile| &profile.id) != Some(&request.profile_id) {
            return Ok(deny_browser_unauthorized(&request.request_id));
        }
        Ok(decide_browser_action(request))
    }
    pub(crate) fn clear_browser_data(
        &self,
        owner: &str,
        request: &BrowserClearDataRequest,
    ) -> Result<BrowserActionResult, AppServiceError> {
        let profile = self
            .store
            .lock()
            .expect("app store should not be poisoned")
            .browser_profile(owner)?;
        if profile.as_ref().map(|profile| &profile.id) != Some(&request.profile_id) {
            return Err(AppServiceError::BrowserProfileForbidden);
        }
        Ok(BrowserActionResult {
            request_id: request.request_id.clone(),
            decision: ta_protocol::wire::BrowserActionDecision::Allow,
            reason: None,
        })
    }
}
