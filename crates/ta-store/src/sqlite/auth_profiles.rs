use rusqlite::{OptionalExtension, params};
use ta_protocol::wire::AuthProfileId;

use crate::{AuthProfileProjection, AuthProfileRepository, SqliteStore, StoreError};

impl AuthProfileRepository for SqliteStore {
    fn auth_profile(
        &self,
        auth_profile_id: &AuthProfileId,
    ) -> Result<Option<AuthProfileProjection>, StoreError> {
        self.conn
            .query_row(
                "SELECT data_json FROM auth_profiles WHERE id = ?1",
                params![auth_profile_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?
            .map(|value| Self::decode("auth profile", value))
            .transpose()
    }

    fn auth_profiles(&self) -> Result<Vec<AuthProfileProjection>, StoreError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT data_json FROM auth_profiles \
                 ORDER BY provider_id, auth_method_id, sort_order, id",
            )
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        rows.map(|row| {
            row.map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })
            .and_then(|value| Self::decode("auth profile", value))
        })
        .collect()
    }

    fn save_auth_profile(&mut self, profile: AuthProfileProjection) -> Result<(), StoreError> {
        let data_json = Self::encode("auth profile", &profile)?;
        self.conn
            .execute(
                "INSERT INTO auth_profiles \
                 (id, auth_method_id, provider_id, sort_order, is_default, data_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(id) DO UPDATE SET \
                 auth_method_id = excluded.auth_method_id, \
                 provider_id = excluded.provider_id, \
                 sort_order = excluded.sort_order, \
                 is_default = excluded.is_default, \
                 data_json = excluded.data_json",
                params![
                    profile.id().as_str(),
                    profile.auth_method_id().as_str(),
                    profile.profile.profile.provider_id.as_str(),
                    i64::from(profile.order),
                    profile.is_default,
                    data_json,
                ],
            )
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })?;
        Ok(())
    }

    fn remove_auth_profile(&mut self, auth_profile_id: &AuthProfileId) -> Result<bool, StoreError> {
        self.conn
            .execute(
                "DELETE FROM auth_profiles WHERE id = ?1",
                params![auth_profile_id.as_str()],
            )
            .map(|changed| changed == 1)
            .map_err(|source| StoreError::PrepareStore {
                path: self.path.clone(),
                source,
            })
    }
}
