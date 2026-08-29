use ta_protocol::wire::{PluginId, PluginInstallation};

use crate::StoreError;

pub trait PluginRepository {
    fn plugin_installations(
        &self,
        owner_principal_id: &str,
    ) -> Result<Vec<PluginInstallation>, StoreError>;
    fn save_plugin_installation(
        &mut self,
        owner_principal_id: &str,
        installation: PluginInstallation,
    ) -> Result<(), StoreError>;
    fn remove_plugin_installation(
        &mut self,
        owner_principal_id: &str,
        plugin_id: &PluginId,
        version: &str,
        digest_sha256: &str,
    ) -> Result<bool, StoreError>;
}
