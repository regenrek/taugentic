use crate::StoreError;
use ta_protocol::wire::{BrowserProfile, BrowserProfileId};

pub trait BrowserProfileRepository {
    fn browser_profile(
        &self,
        owner_principal_id: &str,
    ) -> Result<Option<BrowserProfile>, StoreError>;
    fn save_browser_profile(
        &mut self,
        owner_principal_id: &str,
        profile: BrowserProfile,
    ) -> Result<(), StoreError>;
}
