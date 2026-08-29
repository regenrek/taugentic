use rusqlite::Connection;
use ta_protocol::wire::{PluginCapability, PluginId, PluginInstallation, PluginLifecycleState};

use super::*;
use crate::{PluginRepository, PrincipalProjection, PrincipalRepository};

fn installation() -> PluginInstallation {
    PluginInstallation {
        plugin_id: PluginId::new("example.plugin").expect("plugin id"),
        version: "1.2.3".to_string(),
        digest_sha256: "a".repeat(64),
        requested_capabilities: vec![PluginCapability::WorkspaceRead],
        granted_capabilities: vec![PluginCapability::WorkspaceRead],
        lifecycle_state: PluginLifecycleState::Disabled,
    }
}

#[test]
fn plugin_installation_is_principal_scoped_and_contains_no_source_path() {
    let path = test_db_path("plugin-installation");
    let mut store = SqliteStore::open(&path).expect("store should open");
    PrincipalRepository::save_principal(
        &mut store,
        PrincipalProjection {
            id: "principal-one".to_string(),
            client_name: "sqlite-tests".to_string(),
            credential_hash: "plugin-credential".to_string(),
        },
    )
    .expect("principal should save");
    PrincipalRepository::save_principal(
        &mut store,
        PrincipalProjection {
            id: "principal-two".to_string(),
            client_name: "sqlite-tests-two".to_string(),
            credential_hash: "plugin-credential-two".to_string(),
        },
    )
    .expect("second principal should save");
    let expected = installation();
    store
        .save_plugin_installation("principal-one", expected.clone())
        .expect("installation should save");
    assert!(
        store
            .save_plugin_installation("principal-one", expected.clone())
            .is_err()
    );
    store
        .save_plugin_installation("principal-two", expected.clone())
        .expect("second principal install should save");
    assert_eq!(
        store
            .plugin_installations("principal-one")
            .expect("installations should load"),
        vec![expected]
    );
    assert!(
        store
            .remove_plugin_installation(
                "principal-one",
                &PluginId::new("example.plugin").expect("plugin id"),
                "1.2.3",
                &"a".repeat(64),
            )
            .expect("remove should succeed")
    );
    assert!(
        store
            .plugin_installations("principal-one")
            .expect("principal one should be empty")
            .is_empty()
    );
    assert_eq!(
        store
            .plugin_installations("principal-two")
            .expect("other principal should retain installation")
            .len(),
        1
    );
    drop(store);
    let connection = Connection::open(&path).expect("database should open");
    let columns = connection
        .prepare("PRAGMA table_info(plugin_installations)")
        .expect("schema should inspect")
        .query_map([], |row| row.get::<_, String>(1))
        .expect("columns should query")
        .collect::<Result<Vec<_>, _>>()
        .expect("columns should decode");
    assert_eq!(
        columns,
        [
            "owner_principal_id",
            "plugin_id",
            "version",
            "digest_sha256",
            "data_json"
        ]
    );
    assert!(!columns.iter().any(|column| column.contains("path")
        || column.contains("secret")
        || column.contains("content")));
    drop(connection);
    let _ = std::fs::remove_file(path);
}
