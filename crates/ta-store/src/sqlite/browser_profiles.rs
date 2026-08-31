use super::SqliteStore;
use crate::{BrowserProfileRepository, StoreError};
use rusqlite::{OptionalExtension, params};
use ta_protocol::wire::BrowserProfile;
impl BrowserProfileRepository for SqliteStore {
    fn browser_profile(&self, owner: &str) -> Result<Option<BrowserProfile>, StoreError> {
        self.conn
            .query_row(
                "SELECT data_json FROM browser_profiles WHERE owner_principal_id = ?",
                [owner],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::QueryStore {
                entity: "browser profile",
                source,
            })?
            .map(|json| Self::decode("browser profile", json))
            .transpose()
    }
    fn save_browser_profile(
        &mut self,
        owner: &str,
        profile: BrowserProfile,
    ) -> Result<(), StoreError> {
        let json = Self::encode("browser profile", &profile)?;
        self.conn.execute("INSERT INTO browser_profiles (owner_principal_id, profile_id, data_json) VALUES (?, ?, ?) ON CONFLICT(owner_principal_id) DO UPDATE SET profile_id=excluded.profile_id, data_json=excluded.data_json", params![owner, profile.id.as_str(), json]).map_err(|source| StoreError::QueryStore { entity: "browser profile", source })?;
        Ok(())
    }
}
