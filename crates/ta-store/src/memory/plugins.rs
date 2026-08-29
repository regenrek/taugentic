use ta_protocol::wire::{PluginId, PluginInstallation};

use crate::{PluginRepository, StoreError};

use super::InMemoryStore;

impl PluginRepository for InMemoryStore {
    fn plugin_installations(
        &self,
        owner_principal_id: &str,
    ) -> Result<Vec<PluginInstallation>, StoreError> {
        Ok(self
            .plugin_installations
            .iter()
            .filter(|((owner, _, _, _), _)| owner == owner_principal_id)
            .map(|(_, installation)| installation.clone())
            .collect())
    }

    fn save_plugin_installation(
        &mut self,
        owner_principal_id: &str,
        installation: PluginInstallation,
    ) -> Result<(), StoreError> {
        let key = (
            owner_principal_id.to_string(),
            installation.plugin_id.clone(),
            installation.version.clone(),
            installation.digest_sha256.clone(),
        );
        if self.plugin_installations.contains_key(&key) {
            return Err(StoreError::DuplicateRecord {
                entity: "plugin installation",
                key: installation.plugin_id.as_str().to_string(),
            });
        }
        self.plugin_installations.insert(key, installation);
        Ok(())
    }

    fn remove_plugin_installation(
        &mut self,
        owner_principal_id: &str,
        plugin_id: &PluginId,
        version: &str,
        digest_sha256: &str,
    ) -> Result<bool, StoreError> {
        Ok(self
            .plugin_installations
            .remove(&(
                owner_principal_id.to_string(),
                plugin_id.clone(),
                version.to_string(),
                digest_sha256.to_string(),
            ))
            .is_some())
    }
}
