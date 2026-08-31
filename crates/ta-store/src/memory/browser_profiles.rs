use super::InMemoryStore;
use crate::{BrowserProfileRepository, StoreError};
use ta_protocol::wire::BrowserProfile;
impl BrowserProfileRepository for InMemoryStore {
    fn browser_profile(&self, owner: &str) -> Result<Option<BrowserProfile>, StoreError> {
        Ok(self.browser_profiles.get(owner).cloned())
    }
    fn save_browser_profile(
        &mut self,
        owner: &str,
        profile: BrowserProfile,
    ) -> Result<(), StoreError> {
        self.browser_profiles.insert(owner.to_string(), profile);
        Ok(())
    }
}
