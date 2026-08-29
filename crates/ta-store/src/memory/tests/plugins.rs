use ta_protocol::wire::{PluginCapability, PluginId, PluginInstallation, PluginLifecycleState};

use super::*;
use crate::PluginRepository;

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
fn plugin_installations_are_principal_scoped_duplicate_safe_and_removable() {
    let mut store = InMemoryStore::current();
    let installed = installation();
    store
        .save_plugin_installation("principal-one", installed.clone())
        .expect("first install");
    assert!(
        store
            .save_plugin_installation("principal-one", installed.clone())
            .is_err()
    );
    store
        .save_plugin_installation("principal-two", installed.clone())
        .expect("second principal install");
    assert_eq!(
        store
            .plugin_installations("principal-one")
            .expect("principal one list"),
        vec![installed.clone()]
    );
    assert_eq!(
        store
            .plugin_installations("principal-two")
            .expect("principal two list"),
        vec![installed.clone()]
    );
    assert!(
        store
            .remove_plugin_installation(
                "principal-one",
                &installed.plugin_id,
                &installed.version,
                &installed.digest_sha256,
            )
            .expect("remove principal one")
    );
    assert!(
        store
            .plugin_installations("principal-one")
            .expect("principal one empty")
            .is_empty()
    );
    assert_eq!(
        store
            .plugin_installations("principal-two")
            .expect("principal two retained"),
        vec![installed]
    );
}
