use super::*;

#[test]
fn principal_lookup_persists_saved_projection() {
    let path = test_db_path("principal-lookup");
    let mut store = SqliteStore::open(&path).expect("store should open");
    let principal = PrincipalProjection {
        id: "principal-1".to_string(),
        client_name: "sqlite-tests".to_string(),
        credential_hash: "credential-hash-1".to_string(),
    };

    PrincipalRepository::save_principal(&mut store, principal.clone())
        .expect("principal should persist");

    let reopened = SqliteStore::open(&path).expect("store should reopen");
    assert_eq!(
        some(reopened.principal_by_credential_hash("credential-hash-1")),
        principal
    );

    let _ = std::fs::remove_file(path);
}
