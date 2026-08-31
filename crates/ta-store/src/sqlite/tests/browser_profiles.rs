use rusqlite::Connection;
use ta_protocol::wire::{BrowserProfile, BrowserProfileId};

use super::*;
use crate::{BrowserProfileRepository, PrincipalProjection, PrincipalRepository};

#[test]
fn browser_profile_reopens_in_current_schema_without_serializing_principal_identity() {
    let path = test_db_path("browser-profile");
    let mut store = SqliteStore::open(&path).expect("store should open");
    PrincipalRepository::save_principal(
        &mut store,
        PrincipalProjection {
            id: "principal-one".into(),
            client_name: "browser-test".into(),
            credential_hash: "browser-credential".into(),
        },
    )
    .expect("principal should save");
    let profile = BrowserProfile {
        id: BrowserProfileId::new("browser-profile").expect("profile id"),
    };
    store
        .save_browser_profile("principal-one", profile.clone())
        .expect("profile should save");
    drop(store);
    let reopened = SqliteStore::open(&path).expect("current schema should reopen");
    assert_eq!(
        reopened
            .browser_profile("principal-one")
            .expect("profile should read"),
        Some(profile)
    );
    drop(reopened);
    let connection = Connection::open(&path).expect("database should open");
    let data: String = connection
        .query_row(
            "SELECT data_json FROM browser_profiles WHERE owner_principal_id = 'principal-one'",
            [],
            |row| row.get(0),
        )
        .expect("profile row");
    assert!(!data.contains("ownerPrincipalId"));
    let _ = std::fs::remove_file(path);
}
