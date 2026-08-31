use ta_protocol::wire::{BrowserProfile, BrowserProfileId};

use super::*;
use crate::BrowserProfileRepository;

#[test]
fn browser_profile_is_principal_scoped_without_exposing_ownership_in_the_profile() {
    let mut store = InMemoryStore::current();
    let profile = BrowserProfile {
        id: BrowserProfileId::new("browser-profile").expect("profile id"),
    };
    store
        .save_browser_profile("principal-one", profile.clone())
        .expect("profile should save");
    assert_eq!(
        store
            .browser_profile("principal-one")
            .expect("profile should read"),
        Some(profile)
    );
    assert_eq!(
        store
            .browser_profile("principal-two")
            .expect("other principal should read"),
        None
    );
}
