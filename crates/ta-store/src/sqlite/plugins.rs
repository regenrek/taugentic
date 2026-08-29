use rusqlite::params;
use ta_protocol::wire::{PluginId, PluginInstallation};

use super::*;
use crate::PluginRepository;

impl PluginRepository for SqliteStore {
    fn plugin_installations(
        &self,
        owner_principal_id: &str,
    ) -> Result<Vec<PluginInstallation>, StoreError> {
        let mut statement = self.conn.prepare("SELECT data_json FROM plugin_installations WHERE owner_principal_id = ? ORDER BY plugin_id, version, digest_sha256").map_err(|source| StoreError::QueryStore { entity: "plugin installations", source })?;
        let rows = statement
            .query_map([owner_principal_id], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::QueryStore {
                entity: "plugin installations",
                source,
            })?;
        rows.map(|row| {
            row.map_err(|source| StoreError::QueryStore {
                entity: "plugin installations",
                source,
            })
            .and_then(|json| Self::decode("plugin installation", json))
        })
        .collect()
    }

    fn save_plugin_installation(
        &mut self,
        owner_principal_id: &str,
        installation: PluginInstallation,
    ) -> Result<(), StoreError> {
        self.conn.execute("INSERT INTO plugin_installations (owner_principal_id, plugin_id, version, digest_sha256, data_json) VALUES (?, ?, ?, ?, ?)", params![owner_principal_id, installation.plugin_id.as_str(), installation.version, installation.digest_sha256, Self::encode("plugin installation", &installation)?]).map_err(|source| match source { rusqlite::Error::SqliteFailure(error, _) if error.code == rusqlite::ErrorCode::ConstraintViolation => StoreError::DuplicateRecord { entity: "plugin installation", key: installation.plugin_id.as_str().to_string() }, other => StoreError::QueryStore { entity: "plugin installation", source: other } })?;
        Ok(())
    }

    fn remove_plugin_installation(
        &mut self,
        owner_principal_id: &str,
        plugin_id: &PluginId,
        version: &str,
        digest_sha256: &str,
    ) -> Result<bool, StoreError> {
        let changed = self.conn.execute("DELETE FROM plugin_installations WHERE owner_principal_id = ? AND plugin_id = ? AND version = ? AND digest_sha256 = ?", params![owner_principal_id, plugin_id.as_str(), version, digest_sha256]).map_err(|source| StoreError::QueryStore { entity: "plugin installation", source })?;
        Ok(changed == 1)
    }
}
